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

/// Case-insensitive check that `s` ends with `suffix`.
fn ends_with_ci(s: &str, suffix: &str) -> bool {
    if s.len() < suffix.len() {
        return false;
    }
    s[s.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
}

/// Parse a single human-readable size string.
pub fn parse_human_size(s: &str) -> Option<u64> {
    let s = s.trim();
    if ends_with_ci(s, "gib") {
        let n = s[..s.len() - 3].trim().parse::<f64>().ok()?;
        Some((n * 1_073_741_824.0) as u64)
    } else if ends_with_ci(s, "mib") {
        let n = s[..s.len() - 3].trim().parse::<f64>().ok()?;
        Some((n * 1_048_576.0) as u64)
    } else if ends_with_ci(s, "kib") {
        let n = s[..s.len() - 3].trim().parse::<f64>().ok()?;
        Some((n * 1_024.0) as u64)
    } else if ends_with_ci(s, "gb") {
        let n = s[..s.len() - 2].trim().parse::<f64>().ok()?;
        Some((n * 1_000_000_000.0) as u64)
    } else if ends_with_ci(s, "mb") {
        let n = s[..s.len() - 2].trim().parse::<f64>().ok()?;
        Some((n * 1_000_000.0) as u64)
    } else if ends_with_ci(s, "kb") {
        let n = s[..s.len() - 2].trim().parse::<f64>().ok()?;
        Some((n * 1_000.0) as u64)
    } else if ends_with_ci(s, "bytes") {
        let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
        digits.parse::<u64>().ok()
    } else if ends_with_ci(s, "g") {
        let n = s[..s.len() - 1].trim().parse::<f64>().ok()?;
        Some((n * 1_000_000_000.0) as u64)
    } else if ends_with_ci(s, "m") {
        let n = s[..s.len() - 1].trim().parse::<f64>().ok()?;
        Some((n * 1_000_000.0) as u64)
    } else if ends_with_ci(s, "k") {
        let n = s[..s.len() - 1].trim().parse::<f64>().ok()?;
        Some((n * 1_000.0) as u64)
    } else if ends_with_ci(s, "b") {
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

/// Parse `tmutil listlocalsnapshots /` output to count reclaimable snapshots.
///
/// Only snapshots tmutil can delete are counted. On modern macOS the lines look
/// like:
///   `com.apple.TimeMachine.2024-03-20-143000.local`
/// Older tooling emitted lines with a `localhost` prefix.
///
/// `com.apple.os.update-*` snapshots are system/OS-update snapshots that
/// `tmutil deletelocalsnapshots` refuses to delete, so they are excluded.
pub fn parse_tmutil_list_output(output: &str) -> u64 {
    output
        .lines()
        .filter(|l| {
            let l = l.trim();
            l.starts_with("com.apple.TimeMachine.") || l.starts_with("localhost")
        })
        .count() as u64
}

/// Parse `docker system df --format '{{.Reclaimable}}'` output.
/// Each line is a reclaimable size (optionally with a `(NN%)` suffix); sum them.
pub fn parse_docker_df_output(output: &str) -> u64 {
    output
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.is_empty() || l == "<unknown>" {
                return None;
            }
            // Strip an optional trailing "(NN%)" portion, e.g. "33.65GB (90%)".
            let size = l.split('(').next().unwrap_or(l).trim();
            parse_human_size(size)
        })
        .sum()
}

/// Parse `docker system df --format '{{json .}}'` output: one JSON object per
/// line with `Type` and a reclaimable size field.
///
/// Returns `None` when no line could be parsed as JSON (caller falls back to
/// the legacy table format). `Local Volumes` are excluded because
/// `docker system prune -a` never removes volumes, so counting them would
/// promise bytes the command cannot reclaim.
pub fn parse_docker_df_json(output: &str) -> Option<u64> {
    let mut total = 0_u64;
    let mut parsed_any = false;
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let kind = value.get("Type").and_then(|v| v.as_str());
        // Docker < 29 exposes `ReclaimableSize`; newer versions expose
        // `Reclaimable` with an optional trailing "(NN%)" share.
        let size = value
            .get("ReclaimableSize")
            .or_else(|| value.get("Reclaimable"))
            .and_then(|v| v.as_str());
        let (Some(kind), Some(size)) = (kind, size) else {
            continue;
        };
        parsed_any = true;
        if kind.eq_ignore_ascii_case("Local Volumes") {
            continue;
        }
        let size = size.split('(').next().unwrap_or(size).trim();
        total = total.saturating_add(parse_human_size(size).unwrap_or(0));
    }
    parsed_any.then_some(total)
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
                       com.apple.TimeMachine.2024-03-19-103000.local\n\
                       Some unrelated header line\n";
        assert_eq!(parse_tmutil_list_output(output), 2);
    }

    #[test]
    fn tmutil_list_excludes_os_update_snapshots() {
        // OS-update snapshots cannot be deleted via tmutil, so they must not
        // count toward the reclaimable estimate.
        let output = "Snapshots for volume group containing disk /:\n\
                       com.apple.os.update-AAD61DF03944D7ECCF4925A8608C939B82777C3FD7B744DAE9A7F5E4CDF32B72\n\
                       com.apple.os.update-MSUPrepareUpdate\n";
        assert_eq!(parse_tmutil_list_output(output), 0);
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
        assert_eq!(parse_docker_df_output("<unknown>\n<unknown>\n"), 0);
    }

    // --- parse_docker_df_json ---

    #[test]
    fn docker_df_json_excludes_local_volumes() {
        let output = concat!(
            r#"{"Type":"Images","TotalCount":"5","Active":"2","Size":"37.4GB","ReclaimableSize":"33.65GB","ReclaimablePercent":"90%"}"#,
            "\n",
            r#"{"Type":"Containers","TotalCount":"12","Active":"3","Size":"2.1GB","ReclaimableSize":"1.2GB","ReclaimablePercent":"57%"}"#,
            "\n",
            r#"{"Type":"Local Volumes","TotalCount":"121","Active":"4","Size":"15.15GB","ReclaimableSize":"15GB","ReclaimablePercent":"99%"}"#,
            "\n",
            r#"{"Type":"Build Cache","TotalCount":"40","Active":"0","Size":"1.5GB","ReclaimableSize":"1.5GB","ReclaimablePercent":"100%"}"#,
            "\n",
        );
        // Images + Containers + Build Cache; volumes are NOT reclaimed by
        // `docker system prune -a`.
        assert_eq!(parse_docker_df_json(output), Some(36_350_000_000));
    }

    #[test]
    fn docker_df_json_modern_field_with_percent_share() {
        // Docker 29+ emits `Reclaimable` with a "(NN%)" share suffix.
        let output = concat!(
            r#"{"Active":"12","Reclaimable":"62.52GB (73%)","Size":"84.91GB","TotalCount":"219","Type":"Images"}"#,
            "\n",
            r#"{"Active":"13","Reclaimable":"100.8GB (86%)","Size":"116.3GB","TotalCount":"1393","Type":"Local Volumes"}"#,
            "\n",
            r#"{"Active":"0","Reclaimable":"17.42GB","Size":"21.09GB","TotalCount":"282","Type":"Build Cache"}"#,
            "\n",
        );
        assert_eq!(parse_docker_df_json(output), Some(79_940_000_000));
    }

    #[test]
    fn docker_df_json_empty_returns_none() {
        assert_eq!(parse_docker_df_json(""), None);
    }

    #[test]
    fn docker_df_json_legacy_table_returns_none() {
        assert_eq!(parse_docker_df_json("1.5GB\n800MB\n"), None);
    }

    #[test]
    fn docker_df_handles_percentage_suffix() {
        // Docker 29.x / OrbStack output includes a "(NN%)" portion on some rows.
        let output = "33.65GB (90%)\n757.9kB (80%)\n11.97GB (65%)\n20.56GB\n";
        let expected = 33_650_000_000 + 757_900 + 11_970_000_000 + 20_560_000_000;
        assert_eq!(parse_docker_df_output(output), expected);
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
