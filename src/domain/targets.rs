use std::borrow::Cow;

use crate::config::target_config::CustomTargetEntry;
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
    CleanTarget {
        name: Cow::Borrowed("Gradle Caches"),
        path: Cow::Borrowed("~/.gradle/caches"),
        description: Cow::Borrowed("Java/Kotlin build cache"),
    },
    CleanTarget {
        name: Cow::Borrowed("Maven Repository"),
        path: Cow::Borrowed("~/.m2/repository"),
        description: Cow::Borrowed("Java dependencies"),
    },
    CleanTarget {
        name: Cow::Borrowed("pip Cache"),
        path: Cow::Borrowed("~/.cache/pip"),
        description: Cow::Borrowed("Python package cache"),
    },
    CleanTarget {
        name: Cow::Borrowed("VS Code Cache"),
        path: Cow::Borrowed("~/.config/Code/Cache"),
        description: Cow::Borrowed("Editor cache files"),
    },
    CleanTarget {
        name: Cow::Borrowed("VS Code CachedData"),
        path: Cow::Borrowed("~/.config/Code/CachedData"),
        description: Cow::Borrowed("Editor cached extensions/data"),
    },
    CleanTarget {
        name: Cow::Borrowed("VS Code CachedExtensions"),
        path: Cow::Borrowed("~/.config/Code/CachedExtensions"),
        description: Cow::Borrowed("Editor extension cache"),
    },
    CleanTarget {
        name: Cow::Borrowed("Docker Build Cache"),
        path: Cow::Borrowed("~/.docker/buildkit"),
        description: Cow::Borrowed("BuildKit layer cache"),
    },
];

pub const EXTRA_CACHES: &[CleanTarget] = &[
    CleanTarget {
        name: Cow::Borrowed("Hugging Face Cache"),
        path: Cow::Borrowed("~/.cache/huggingface"),
        description: Cow::Borrowed("ML model downloads"),
    },
    CleanTarget {
        name: Cow::Borrowed("Ollama Models"),
        path: Cow::Borrowed("~/.ollama/models"),
        description: Cow::Borrowed("Local LLM models"),
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
    CleanTarget {
        name: Cow::Borrowed("Homebrew Cache"),
        path: Cow::Borrowed("~/Library/Caches/Homebrew"),
        description: Cow::Borrowed("Formula downloads"),
    },
    CleanTarget {
        name: Cow::Borrowed("iOS DeviceSupport"),
        path: Cow::Borrowed("~/Library/Developer/Xcode/iOS DeviceSupport"),
        description: Cow::Borrowed("Device debug symbols"),
    },
    CleanTarget {
        name: Cow::Borrowed("Firefox Cache"),
        path: Cow::Borrowed("~/Library/Caches/Firefox"),
        description: Cow::Borrowed("Browser cache"),
    },
    CleanTarget {
        name: Cow::Borrowed("Chrome Cache"),
        path: Cow::Borrowed("~/Library/Caches/Google/Chrome"),
        description: Cow::Borrowed("Google Chrome cache"),
    },
    CleanTarget {
        name: Cow::Borrowed("Chromium Cache"),
        path: Cow::Borrowed("~/Library/Caches/Chromium"),
        description: Cow::Borrowed("Chromium browser cache"),
    },
    CleanTarget {
        name: Cow::Borrowed("Brave Cache"),
        path: Cow::Borrowed("~/Library/Caches/BraveSoftware/Brave-Browser"),
        description: Cow::Borrowed("Brave browser cache"),
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
    CleanTarget {
        name: Cow::Borrowed("Pacman Cache"),
        path: Cow::Borrowed("/var/cache/pacman/pkg"),
        description: Cow::Borrowed("Arch package cache (Requires sudo)"),
    },
    CleanTarget {
        name: Cow::Borrowed("Yay Cache"),
        path: Cow::Borrowed("~/.cache/yay"),
        description: Cow::Borrowed("AUR helper build cache"),
    },
    CleanTarget {
        name: Cow::Borrowed("Paru Cache"),
        path: Cow::Borrowed("~/.cache/paru"),
        description: Cow::Borrowed("AUR helper build cache"),
    },
    CleanTarget {
        name: Cow::Borrowed("Flatpak Cache"),
        path: Cow::Borrowed("~/.cache/flatpak"),
        description: Cow::Borrowed("Flatpak app downloads"),
    },
    CleanTarget {
        name: Cow::Borrowed("Snap Cache"),
        path: Cow::Borrowed("/var/lib/snapd/cache"),
        description: Cow::Borrowed("Snap package cache (Requires sudo)"),
    },
    CleanTarget {
        name: Cow::Borrowed("Docker Overlay2"),
        path: Cow::Borrowed("/var/lib/docker/overlay2"),
        description: Cow::Borrowed("Docker container layers (Requires sudo)"),
    },
    CleanTarget {
        name: Cow::Borrowed("Firefox Cache"),
        path: Cow::Borrowed("~/.cache/mozilla/firefox"),
        description: Cow::Borrowed("Browser cache"),
    },
    CleanTarget {
        name: Cow::Borrowed("Chrome Cache"),
        path: Cow::Borrowed("~/.cache/google-chrome"),
        description: Cow::Borrowed("Google Chrome cache"),
    },
    CleanTarget {
        name: Cow::Borrowed("Chromium Cache"),
        path: Cow::Borrowed("~/.cache/chromium"),
        description: Cow::Borrowed("Chromium browser cache"),
    },
    CleanTarget {
        name: Cow::Borrowed("Brave Cache"),
        path: Cow::Borrowed("~/.cache/BraveSoftware/Brave-Browser"),
        description: Cow::Borrowed("Brave browser cache"),
    },
];

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub const OS_CACHES: &[CleanTarget] = &[];

pub fn build_targets(
    filter_names: &[String],
    custom_targets: &[CustomTargetEntry],
) -> Vec<CleanTarget> {
    let custom: Vec<CleanTarget> = custom_targets
        .iter()
        .map(|ct| CleanTarget {
            name: Cow::Owned(ct.name.clone()),
            path: Cow::Owned(ct.path.clone()),
            description: Cow::Owned(ct.description.clone()),
        })
        .collect();

    let mut all = Vec::with_capacity(
        DEV_CACHES.len() + OS_CACHES.len() + EXTRA_CACHES.len() + custom.len(),
    );
    all.extend_from_slice(DEV_CACHES);
    all.extend(
        OS_CACHES
            .iter()
            .filter(|t| t.resolved_path().exists())
            .cloned(),
    );
    all.extend(
        EXTRA_CACHES
            .iter()
            .filter(|t| t.resolved_path().exists())
            .cloned(),
    );
    all.extend(custom);

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
    use crate::config::target_config::CustomTargetEntry;

    #[test]
    fn returns_all_when_filter_is_empty() {
        let targets = build_targets(&[], &[]);
        assert!(!targets.is_empty());
    }

    #[test]
    fn filters_by_name_case_insensitive() {
        let targets = build_targets(&["cargo registry".to_string()], &[]);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].name, "Cargo Registry");
    }

    #[test]
    fn includes_custom_targets() {
        let custom = vec![CustomTargetEntry {
            name: String::from("My Custom"),
            path: String::from("/tmp/custom"),
            description: String::from("test"),
        }];
        let targets = build_targets(&[], &custom);
        assert!(targets.iter().any(|t| t.name == "My Custom"));
        assert!(targets.len() > 1);
    }
}
