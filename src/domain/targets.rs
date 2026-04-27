use std::borrow::Cow;

use crate::domain::CleanTarget;

pub const DEV_CACHES: &[CleanTarget] = &[
    CleanTarget {
        name: Cow::Borrowed("Cargo Registry"),
        path: Cow::Borrowed("~/.cargo/registry"),
        description: Cow::Borrowed("Rust package cache"),
    },
    CleanTarget {
        name: Cow::Borrowed("Go Module Cache"),
        path: Cow::Borrowed("~/go/pkg/mod"),
        description: Cow::Borrowed("Go dependencies"),
    },
    CleanTarget {
        name: Cow::Borrowed("NPM Cache"),
        path: Cow::Borrowed("~/.npm"),
        description: Cow::Borrowed("Node.js package cache"),
    },
    CleanTarget {
        name: Cow::Borrowed("Yarn Cache"),
        path: Cow::Borrowed("~/.cache/yarn"),
        description: Cow::Borrowed("Yarn package cache"),
    },
];

#[cfg(target_os = "macos")]
pub const OS_CACHES: &[CleanTarget] = &[
    CleanTarget {
        name: Cow::Borrowed("Xcode DerivedData"),
        path: Cow::Borrowed("~/Library/Developer/Xcode/DerivedData"),
        description: Cow::Borrowed("Xcode build artifacts"),
    },
    CleanTarget {
        name: Cow::Borrowed("iOS Simulators"),
        path: Cow::Borrowed("~/Library/Developer/CoreSimulator/Caches"),
        description: Cow::Borrowed("Simulator app caches"),
    },
    CleanTarget {
        name: Cow::Borrowed("User Caches"),
        path: Cow::Borrowed("~/Library/Caches"),
        description: Cow::Borrowed("General application caches"),
    },
    CleanTarget {
        name: Cow::Borrowed("User Logs"),
        path: Cow::Borrowed("~/Library/Logs"),
        description: Cow::Borrowed("Application crash logs and telemetry"),
    },
    CleanTarget {
        name: Cow::Borrowed("Trash"),
        path: Cow::Borrowed("~/.Trash"),
        description: Cow::Borrowed("User trash bin"),
    },
];

#[cfg(target_os = "linux")]
pub const OS_CACHES: &[CleanTarget] = &[
    CleanTarget {
        name: Cow::Borrowed("User Caches"),
        path: Cow::Borrowed("~/.cache"),
        description: Cow::Borrowed("Standard XDG cache directory"),
    },
    CleanTarget {
        name: Cow::Borrowed("Thumbnail Cache"),
        path: Cow::Borrowed("~/.cache/thumbnails"),
        description: Cow::Borrowed("Image explorer thumbnails"),
    },
    CleanTarget {
        name: Cow::Borrowed("Systemd Journal Logs"),
        path: Cow::Borrowed("/var/log/journal"),
        description: Cow::Borrowed("Systemd binary logs (Requires sudo)"),
    },
    CleanTarget {
        name: Cow::Borrowed("Apt Cache"),
        path: Cow::Borrowed("/var/cache/apt/archives"),
        description: Cow::Borrowed("Debian/Ubuntu package cache (Requires sudo)"),
    },
    CleanTarget {
        name: Cow::Borrowed("Trash"),
        path: Cow::Borrowed("~/.local/share/Trash"),
        description: Cow::Borrowed("XDG trash bin"),
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
