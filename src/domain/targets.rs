use crate::domain::CleanTarget;

pub const DEV_CACHES: &[CleanTarget] = &[
    CleanTarget {
        name: "Cargo Registry",
        path: "~/.cargo/registry",
        description: "Rust package cache",
    },
    CleanTarget {
        name: "Go Module Cache",
        path: "~/go/pkg/mod",
        description: "Go dependencies",
    },
    CleanTarget {
        name: "NPM Cache",
        path: "~/.npm",
        description: "Node.js package cache",
    },
    CleanTarget {
        name: "Yarn Cache",
        path: "~/.cache/yarn",
        description: "Yarn package cache",
    },
];

#[cfg(target_os = "macos")]
pub const OS_CACHES: &[CleanTarget] = &[
    CleanTarget {
        name: "Xcode DerivedData",
        path: "~/Library/Developer/Xcode/DerivedData",
        description: "Xcode build artifacts",
    },
    CleanTarget {
        name: "iOS Simulators",
        path: "~/Library/Developer/CoreSimulator/Caches",
        description: "Simulator app caches",
    },
    CleanTarget {
        name: "User Caches",
        path: "~/Library/Caches",
        description: "General application caches",
    },
    CleanTarget {
        name: "User Logs",
        path: "~/Library/Logs",
        description: "Application crash logs and telemetry",
    },
    CleanTarget {
        name: "Trash",
        path: "~/.Trash",
        description: "User trash bin",
    },
];

#[cfg(target_os = "linux")]
pub const OS_CACHES: &[CleanTarget] = &[
    CleanTarget {
        name: "User Caches",
        path: "~/.cache",
        description: "Standard XDG cache directory",
    },
    CleanTarget {
        name: "Thumbnail Cache",
        path: "~/.cache/thumbnails",
        description: "Image explorer thumbnails",
    },
    CleanTarget {
        name: "Systemd Journal Logs",
        path: "/var/log/journal",
        description: "Systemd binary logs (Requires sudo)",
    },
    CleanTarget {
        name: "Apt Cache",
        path: "/var/cache/apt/archives",
        description: "Debian/Ubuntu package cache (Requires sudo)",
    },
    CleanTarget {
        name: "Trash",
        path: "~/.local/share/Trash",
        description: "XDG trash bin",
    },
];

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub const OS_CACHES: &[CleanTarget] = &[];

pub fn build_targets(filter_names: &[String]) -> Vec<CleanTarget> {
    let mut all = Vec::with_capacity(DEV_CACHES.len() + OS_CACHES.len());
    all.extend_from_slice(DEV_CACHES);
    all.extend_from_slice(OS_CACHES);

    if filter_names.is_empty() {
        return all;
    }

    all.into_iter()
        .filter(|target| {
            filter_names
                .iter()
                .any(|name| target.name.eq_ignore_ascii_case(name))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::build_targets;

    #[test]
    fn returns_all_when_filter_is_empty() {
        let targets = build_targets(&[]);
        assert!(!targets.is_empty());
    }

    #[test]
    fn filters_by_name_case_insensitive() {
        let targets = build_targets(&["cargo registry".to_string()]);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].name, "Cargo Registry");
    }
}
