use std::process::Output;

pub fn run_command(cmd: &[&str]) -> Result<Output, String> {
    if cmd.is_empty() {
        return Err("empty command".into());
    }
    std::process::Command::new(cmd[0])
        .args(&cmd[1..])
        .output()
        .map_err(|e| format!("failed to run {}: {e}", cmd[0]))
}

pub fn run_command_get_stdout(cmd: &[&str]) -> Result<String, String> {
    let output = run_command(cmd)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("command failed: {stderr}"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Parse "Purgeable: N bytes" or "Purgeable: N.N GB/GiB" from `diskutil info /`.
pub fn parse_purgeable_bytes(output: &str) -> Option<u64> {
    for line in output.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("Purgeable:") {
            return parse_single_size(val.trim());
        }
    }
    None
}

/// Parse a single human-readable size string.
pub fn parse_human_size(s: &str) -> Option<u64> {
    let s = s.trim();
    let lower = s.to_lowercase();
    if lower.ends_with("gib") {
        let n = s[..s.len() - 3].trim().parse::<f64>().ok()?;
        Some((n * 1_073_741_824.0) as u64)
    } else if lower.ends_with("mib") {
        let n = s[..s.len() - 3].trim().parse::<f64>().ok()?;
        Some((n * 1_048_576.0) as u64)
    } else if lower.ends_with("kib") {
        let n = s[..s.len() - 3].trim().parse::<f64>().ok()?;
        Some((n * 1_024.0) as u64)
    } else if lower.ends_with("gb") {
        let n = s[..s.len() - 2].trim().parse::<f64>().ok()?;
        Some((n * 1_000_000_000.0) as u64)
    } else if lower.ends_with("mb") {
        let n = s[..s.len() - 2].trim().parse::<f64>().ok()?;
        Some((n * 1_000_000.0) as u64)
    } else if lower.ends_with("kb") {
        let n = s[..s.len() - 2].trim().parse::<f64>().ok()?;
        Some((n * 1_000.0) as u64)
    } else if lower.ends_with('g') {
        let n = s[..s.len() - 1].trim().parse::<f64>().ok()?;
        Some((n * 1_000_000_000.0) as u64)
    } else if lower.ends_with('m') {
        let n = s[..s.len() - 1].trim().parse::<f64>().ok()?;
        Some((n * 1_000_000.0) as u64)
    } else if lower.ends_with('k') {
        let n = s[..s.len() - 1].trim().parse::<f64>().ok()?;
        Some((n * 1_000.0) as u64)
    } else if lower.ends_with("bytes") {
        let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
        digits.parse::<u64>().ok()
    } else if lower.ends_with('b') {
        // bare "b" suffix often means bytes
        s[..s.len() - 1].trim().parse::<u64>().ok()
    } else {
        s.parse::<u64>().ok()
    }
}

fn parse_single_size(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(bytes_str) = s.strip_suffix("bytes") {
        let cleaned: String = bytes_str.chars().filter(|c| c.is_ascii_digit()).collect();
        return cleaned.parse::<u64>().ok();
    }
    parse_human_size(s)
}

/// Parse `tmutil listlocalsnapshots /` output to count snapshots.
///
/// On modern macOS the snapshot lines look like:
///   `com.apple.TimeMachine.2024-03-20-143000.local`
///   `com.apple.os.update-<UUID>`
/// Older tooling emitted lines with a `localhost` prefix.
pub fn parse_tmutil_list_output(output: &str) -> u64 {
    output
        .lines()
        .filter(|l| {
            let l = l.trim();
            l.starts_with("com.apple.TimeMachine.")
                || l.starts_with("com.apple.os.update")
                || l.starts_with("com.apple.")
                || l.starts_with("localhost")
        })
        .count() as u64
}

/// Parse `docker system df --format '{{.ReclaimableSize}}'` output.
/// Each line is a reclaimable size; sum them up.
pub fn parse_docker_df_output(output: &str) -> u64 {
    output
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.is_empty() || l == "<unknown>" {
                return None;
            }
            parse_human_size(l)
        })
        .sum()
}

/// Parse `journalctl --disk-usage` output like "Archived and active journals use 1.2G."
pub fn parse_journalctl_output(output: &str) -> Option<u64> {
    let line = output.lines().find(|l| l.contains("use"))?;
    let word = line.split_whitespace().find(|w| {
        let clean = w.trim_end_matches(|c: char| c.is_ascii_punctuation());
        clean.ends_with('G')
            || clean.ends_with('M')
            || clean.ends_with('K')
            || clean.ends_with("GB")
            || clean.ends_with("MB")
            || clean.ends_with("KB")
    })?;
    let clean = word.trim_end_matches(|c: char| c.is_ascii_punctuation());
    parse_human_size(clean)
}

/// Parse `apt --just-print autoremove` output.
/// Counts lines starting with "Purg" as packages to be removed.
pub fn parse_apt_autoremove_output(output: &str) -> (u64, u64) {
    let count = output.lines().filter(|l| l.starts_with("Purg")).count() as u64;
    (count.saturating_mul(10_000_000), count)
}

/// Parse `diskutil info /` output to get purgeable bytes.
pub fn parse_diskutil_info_output(output: &str) -> Option<u64> {
    parse_purgeable_bytes(output)
}

/// Query the APFS purgeable space via `diskutil info /` on macOS.
/// Returns `None` on non-macOS or when the value is unavailable.
pub fn purgeable_bytes() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        let stdout = run_command_get_stdout(&["diskutil", "info", "/"]).ok()?;
        parse_purgeable_bytes(&stdout)
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_human_size ---

    #[test]
    fn parse_human_size_gib() {
        assert_eq!(parse_human_size("2.5 GiB"), Some(2_684_354_560));
    }

    #[test]
    fn parse_human_size_mib() {
        assert_eq!(parse_human_size("1.5 MiB"), Some(1_572_864));
    }

    #[test]
    fn parse_human_size_kib() {
        assert_eq!(parse_human_size("512 KiB"), Some(524_288));
    }

    #[test]
    fn parse_human_size_gb() {
        assert_eq!(parse_human_size("5.2 GB"), Some(5_200_000_000));
    }

    #[test]
    fn parse_human_size_mb() {
        assert_eq!(parse_human_size("800 MB"), Some(800_000_000));
    }

    #[test]
    fn parse_human_size_kb() {
        assert_eq!(parse_human_size("100 KB"), Some(100_000));
    }

    #[test]
    fn parse_human_size_bare_g() {
        assert_eq!(parse_human_size("1.2G"), Some(1_200_000_000));
    }

    #[test]
    fn parse_human_size_bare_m() {
        assert_eq!(parse_human_size("500M"), Some(500_000_000));
    }

    #[test]
    fn parse_human_size_bare_k() {
        assert_eq!(parse_human_size("64K"), Some(64_000));
    }

    #[test]
    fn parse_human_size_raw_bytes() {
        assert_eq!(parse_human_size("12345678"), Some(12_345_678));
    }

    #[test]
    fn parse_human_size_bytes_suffix() {
        assert_eq!(parse_human_size("1024 bytes"), Some(1024));
    }

    #[test]
    fn parse_human_size_empty() {
        assert_eq!(parse_human_size(""), None);
    }

    // --- parse_purgeable_bytes ---

    #[test]
    fn purgeable_full_bytes() {
        let input = "   Purgeable:   51,234,567,890 bytes\n";
        assert_eq!(parse_purgeable_bytes(input), Some(51_234_567_890));
    }

    #[test]
    fn purgeable_gb() {
        let input = "   Purgeable:   5.2 GB\n";
        assert_eq!(parse_purgeable_bytes(input), Some(5_200_000_000));
    }

    #[test]
    fn purgeable_gib() {
        let input = "   Purgeable:   2.5 GiB\n";
        assert_eq!(parse_purgeable_bytes(input), Some(2_684_354_560));
    }

    #[test]
    fn purgeable_no_match() {
        let input = "   Something:  1234 bytes\n";
        assert_eq!(parse_purgeable_bytes(input), None);
    }

    #[test]
    fn purgeable_empty() {
        assert_eq!(parse_purgeable_bytes(""), None);
    }

    // --- parse_tmutil_list_output ---

    #[test]
    fn tmutil_list_counts_snapshots() {
        let output = "Snapshots for volume group containing disk /:\n\
                       localhost 2025-01-15-123456\n\
                       localhost 2025-01-14-123456\n";
        assert_eq!(parse_tmutil_list_output(output), 2);
    }

    #[test]
    fn tmutil_list_counts_apfs_snapshots() {
        let output = "Snapshots for volume group containing disk /:\n\
                       com.apple.TimeMachine.2024-03-20-143000.local\n\
                       com.apple.os.update-AAD61DF03944D7ECCF4925A8608C939B82777C3FD7B744DAE9A7F5E4CDF32B72\n\
                       Some unrelated header line\n";
        assert_eq!(parse_tmutil_list_output(output), 2);
    }

    #[test]
    fn tmutil_list_no_snapshots() {
        assert_eq!(parse_tmutil_list_output("No local snapshots found\n"), 0);
    }

    // --- parse_docker_df_output ---

    #[test]
    fn docker_df_sums_reclaimable() {
        let output = "1.5GB\n800MB\n2.3GB\n<unknown>\n";
        assert_eq!(parse_docker_df_output(output), 4_600_000_000);
    }

    #[test]
    fn docker_df_empty() {
        assert_eq!(parse_docker_df_output(""), 0);
    }

    #[test]
    fn docker_df_all_unknown() {
        let output = "<unknown>\n<unknown>\n";
        assert_eq!(parse_docker_df_output(output), 0);
    }

    // --- parse_journalctl_output ---

    #[test]
    fn journalctl_parses_usage() {
        let output = "Archived and active journals use 1.2G.\n";
        assert_eq!(parse_journalctl_output(output), Some(1_200_000_000));
    }

    #[test]
    fn journalctl_no_match() {
        let output = "No journals found.\n";
        assert_eq!(parse_journalctl_output(output), None);
    }

    // --- parse_apt_autoremove_output ---

    #[test]
    fn apt_autoremove_counts_packages() {
        let output = "Reading package lists...\n\
                       Purg libfoo [1.0]\n\
                       Purg libbar [2.0]\n";
        let (bytes, count) = parse_apt_autoremove_output(output);
        assert_eq!(count, 2);
        assert!(bytes > 0);
    }

    #[test]
    fn apt_autoremove_nothing() {
        let output = "Reading package lists...\n0 upgraded, 0 newly installed, 0 to remove\n";
        let (bytes, count) = parse_apt_autoremove_output(output);
        assert_eq!(count, 0);
        assert_eq!(bytes, 0);
    }

    // --- parse_diskutil_info_output ---

    #[test]
    fn diskutil_info_purgeable() {
        let input = "   Purgeable:   12,345,678,912 bytes\n";
        assert_eq!(parse_diskutil_info_output(input), Some(12_345_678_912));
    }

    #[test]
    fn diskutil_info_no_purgeable() {
        assert_eq!(parse_diskutil_info_output(""), None);
    }

    // --- run_command with echo (real integration) ---

    #[test]
    fn run_command_echo_succeeds() {
        let out = run_command_get_stdout(&["echo", "hello"]).unwrap();
        assert_eq!(out.trim(), "hello");
    }

    #[test]
    fn run_command_nonexistent_fails() {
        let result = run_command_get_stdout(&["nonexistent-command-12345"]);
        assert!(result.is_err());
    }

    #[test]
    fn run_command_empty_fails() {
        let result = run_command_get_stdout(&[]);
        assert!(result.is_err());
    }
}
