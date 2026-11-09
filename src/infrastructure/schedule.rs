//! Background jobs: an hourly `acari check --notify` and a weekly
//! `acari guide --background`, as systemd user timers (Linux) or launchd
//! agents (macOS).

use std::path::{Path, PathBuf};

use super::changes::Change;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Job {
    /// Hourly free-space check with a desktop notification when low.
    Check,
    /// Weekly low-priority refresh of the saved guide report.
    Report,
}

impl Job {
    fn slug(self) -> &'static str {
        match self {
            Job::Check => "check",
            Job::Report => "report",
        }
    }

    fn args(self) -> &'static [&'static str] {
        match self {
            Job::Check => &["check", "--notify"],
            Job::Report => &["guide", "--background"],
        }
    }
}

/// Whether background jobs can be installed on this platform.
pub fn supported() -> bool {
    cfg!(target_os = "macos") || (cfg!(target_os = "linux") && super::agents::on_path("systemctl"))
}

/// Whether desktop notifications can be shown.
pub fn notifier_available() -> bool {
    if cfg!(target_os = "macos") {
        true
    } else {
        super::agents::on_path("notify-send")
    }
}

// ---------------------------------------------------------------- systemd

fn systemd_dir(home: &Path) -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"))
        .join("systemd/user")
}

fn systemd_unit(job: Job) -> String {
    format!("acari-{}", job.slug())
}

fn systemd_exec(exe: &Path, job: Job) -> String {
    // systemd's own quoting: wrap paths with spaces in double quotes.
    let exe = exe.display().to_string();
    let exe = if exe.contains(' ') {
        format!("\"{exe}\"")
    } else {
        exe
    };
    format!("{exe} {}", job.args().join(" "))
}

pub fn systemd_service(exe: &Path, job: Job) -> String {
    let (description, extra) = match job {
        Job::Check => ("acari: low disk space check", ""),
        Job::Report => (
            "acari: refresh the disk usage report",
            "Nice=19\nIOSchedulingClass=idle\n",
        ),
    };
    format!(
        "# Managed by acari (acari setup reminders)\n[Unit]\nDescription={description}\n\n[Service]\nType=oneshot\nExecStart={}\n{extra}",
        systemd_exec(exe, job)
    )
}

pub fn systemd_timer(job: Job) -> String {
    let (calendar, delay) = match job {
        Job::Check => ("hourly", "120"),
        Job::Report => ("weekly", "3600"),
    };
    format!(
        "# Managed by acari (acari setup reminders)\n[Unit]\nDescription=Run {unit} periodically\n\n[Timer]\nOnCalendar={calendar}\nPersistent=true\nRandomizedDelaySec={delay}\n\n[Install]\nWantedBy=timers.target\n",
        unit = systemd_unit(job)
    )
}

// ---------------------------------------------------------------- launchd

fn launchd_dir(home: &Path) -> PathBuf {
    home.join("Library/LaunchAgents")
}

fn launchd_label(job: Job) -> String {
    format!("dev.acari.{}", job.slug())
}

fn xml_escape(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn launchd_plist(exe: &Path, job: Job) -> String {
    let mut args = format!(
        "    <string>{}</string>\n",
        xml_escape(&exe.display().to_string())
    );
    for a in job.args() {
        args.push_str(&format!("    <string>{a}</string>\n"));
    }
    let schedule = match job {
        Job::Check => "  <key>StartInterval</key>\n  <integer>3600</integer>\n".to_string(),
        Job::Report => "  <key>StartCalendarInterval</key>\n  <dict>\n    <key>Weekday</key>\n    <integer>0</integer>\n    <key>Hour</key>\n    <integer>12</integer>\n  </dict>\n  <key>LowPriorityIO</key>\n  <true/>\n  <key>Nice</key>\n  <integer>19</integer>\n".to_string(),
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<!-- Managed by acari (acari setup reminders) -->\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key>\n  <string>{label}</string>\n  <key>ProgramArguments</key>\n  <array>\n{args}  </array>\n{schedule}  <key>ProcessType</key>\n  <string>Background</string>\n</dict>\n</plist>\n",
        label = launchd_label(job)
    )
}

fn launchd_domain() -> String {
    // SAFETY: getuid has no preconditions and cannot fail.
    #[cfg(unix)]
    let uid = unsafe { libc::getuid() };
    #[cfg(not(unix))]
    let uid = 0;
    format!("gui/{uid}")
}

// ---------------------------------------------------------------- plans

fn job_files(home: &Path, job: Job) -> Vec<PathBuf> {
    if cfg!(target_os = "macos") {
        vec![launchd_dir(home).join(format!("{}.plist", launchd_label(job)))]
    } else {
        let dir = systemd_dir(home);
        let unit = systemd_unit(job);
        vec![
            dir.join(format!("{unit}.service")),
            dir.join(format!("{unit}.timer")),
        ]
    }
}

pub fn is_installed(home: &Path, job: Job) -> bool {
    job_files(home, job).iter().all(|p| p.is_file())
}

fn run(program: &str, args: &[&str], allow_failure: bool) -> Change {
    Change::Run {
        program: program.into(),
        args: args.iter().map(|a| a.to_string()).collect(),
        allow_failure,
    }
}

pub fn plan_install(home: &Path, job: Job, exe: &Path) -> Vec<Change> {
    if cfg!(target_os = "macos") {
        let path = job_files(home, job).remove(0);
        let target = path.display().to_string();
        let domain = launchd_domain();
        vec![
            run("launchctl", &["bootout", &domain, &target], true),
            Change::WriteFile {
                path,
                content: launchd_plist(exe, job),
                summary: format!("launchd agent {}", launchd_label(job)),
            },
            run("launchctl", &["bootstrap", &domain, &target], false),
        ]
    } else {
        let files = job_files(home, job);
        let unit = systemd_unit(job);
        vec![
            Change::WriteFile {
                path: files[0].clone(),
                content: systemd_service(exe, job),
                summary: format!("systemd service ({})", job.args().join(" ")),
            },
            Change::WriteFile {
                path: files[1].clone(),
                content: systemd_timer(job),
                summary: "systemd timer".into(),
            },
            run("systemctl", &["--user", "daemon-reload"], false),
            run(
                "systemctl",
                &["--user", "enable", "--now", &format!("{unit}.timer")],
                false,
            ),
        ]
    }
}

pub fn plan_remove(home: &Path, job: Job) -> Vec<Change> {
    if !is_installed(home, job) {
        return Vec::new();
    }
    let files = job_files(home, job);
    let mut changes = if cfg!(target_os = "macos") {
        vec![run(
            "launchctl",
            &[
                "bootout",
                &launchd_domain(),
                &files[0].display().to_string(),
            ],
            true,
        )]
    } else {
        vec![run(
            "systemctl",
            &[
                "--user",
                "disable",
                "--now",
                &format!("{}.timer", systemd_unit(job)),
            ],
            true,
        )]
    };
    changes.extend(files.into_iter().map(|path| Change::RemoveFile { path }));
    if !cfg!(target_os = "macos") {
        changes.push(run("systemctl", &["--user", "daemon-reload"], true));
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn systemd_units_run_the_right_commands() {
        let exe = Path::new("/home/u/.local/bin/acari");
        let check = systemd_service(exe, Job::Check);
        assert!(check.contains("ExecStart=/home/u/.local/bin/acari check --notify"));
        let report = systemd_service(exe, Job::Report);
        assert!(report.contains("guide --background"));
        assert!(report.contains("IOSchedulingClass=idle"));
        assert!(systemd_timer(Job::Check).contains("OnCalendar=hourly"));
        assert!(systemd_timer(Job::Report).contains("OnCalendar=weekly"));
        let spaced = systemd_service(Path::new("/a b/acari"), Job::Check);
        assert!(spaced.contains("ExecStart=\"/a b/acari\" check --notify"));
    }

    #[test]
    fn launchd_plist_is_escaped_and_scheduled() {
        let plist = launchd_plist(Path::new("/Users/u/A&B/acari"), Job::Report);
        assert!(plist.contains("<string>/Users/u/A&amp;B/acari</string>"));
        assert!(plist.contains("<string>--background</string>"));
        assert!(plist.contains("StartCalendarInterval"));
        assert!(launchd_plist(Path::new("/a"), Job::Check).contains("<integer>3600</integer>"));
    }

    #[test]
    fn install_then_remove_plans_cover_every_file() {
        if std::env::var_os("XDG_CONFIG_HOME").is_some() {
            return; // units would land outside the temp home
        }
        let home = tempfile::tempdir().unwrap();
        let exe = Path::new("/usr/local/bin/acari");
        let plan = plan_install(home.path(), Job::Check, exe);
        for change in &plan {
            if matches!(change, Change::WriteFile { .. }) {
                change.apply().unwrap();
            }
        }
        assert!(is_installed(home.path(), Job::Check));
        assert!(!is_installed(home.path(), Job::Report));
        let removal = plan_remove(home.path(), Job::Check);
        for change in &removal {
            if matches!(change, Change::RemoveFile { .. }) {
                change.apply().unwrap();
            }
        }
        assert!(!is_installed(home.path(), Job::Check));
    }
}
