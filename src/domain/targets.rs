use std::borrow::Cow;

use crate::config::target_config::CustomTargetEntry;
use crate::domain::CleanTarget;

pub const DEV_CACHES: &[CleanTarget] = &[
    CleanTarget {
        name: Cow::Borrowed("Cargo Registry"),
        path: Cow::Borrowed("~/.cargo/registry"),
        description: Cow::Borrowed("Rust package cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Go Module Cache"),
        path: Cow::Borrowed("~/go/pkg/mod"),
        description: Cow::Borrowed("Go dependencies"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("NPM Cache"),
        path: Cow::Borrowed("~/.npm"),
        description: Cow::Borrowed("Node.js package cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Yarn Cache"),
        path: Cow::Borrowed("~/.cache/yarn"),
        description: Cow::Borrowed("Yarn package cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Gradle Caches"),
        path: Cow::Borrowed("~/.gradle/caches"),
        description: Cow::Borrowed("Java/Kotlin build cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Maven Repository"),
        path: Cow::Borrowed("~/.m2/repository"),
        description: Cow::Borrowed("Java dependencies"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Docker Build Cache"),
        path: Cow::Borrowed("~/.docker/buildkit"),
        description: Cow::Borrowed("BuildKit layer cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
];

pub const EXTRA_CACHES: &[CleanTarget] = &[
    CleanTarget {
        name: Cow::Borrowed("Hugging Face Cache"),
        path: Cow::Borrowed("~/.cache/huggingface"),
        description: Cow::Borrowed("ML model downloads"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Ollama Models"),
        path: Cow::Borrowed("~/.ollama/models"),
        description: Cow::Borrowed("Local LLM models"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
];

#[cfg(target_os = "macos")]
pub const OS_CACHES: &[CleanTarget] = &[
    CleanTarget {
        name: Cow::Borrowed("Go Build Cache"),
        path: Cow::Borrowed("~/Library/Caches/go-build"),
        description: Cow::Borrowed("Go compiler build cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("pnpm Cache"),
        path: Cow::Borrowed("~/Library/Caches/pnpm"),
        description: Cow::Borrowed("pnpm package store"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Yarn Cache"),
        path: Cow::Borrowed("~/Library/Caches/Yarn"),
        description: Cow::Borrowed("Yarn package cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("pip Cache"),
        path: Cow::Borrowed("~/Library/Caches/pip"),
        description: Cow::Borrowed("Python package cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Playwright Browsers"),
        path: Cow::Borrowed("~/Library/Caches/ms-playwright"),
        description: Cow::Borrowed("Playwright browser downloads"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("JetBrains IDE Caches"),
        path: Cow::Borrowed("~/Library/Caches/JetBrains"),
        description: Cow::Borrowed("JetBrains IDE caches and indexes"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Google IDE Caches"),
        path: Cow::Borrowed("~/Library/Caches/Google"),
        description: Cow::Borrowed("Google/Android Studio caches (all versions)"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("VS Code Cache"),
        path: Cow::Borrowed("~/Library/Application Support/Code/Cache"),
        description: Cow::Borrowed("Editor cache files"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("VS Code CachedData"),
        path: Cow::Borrowed("~/Library/Application Support/Code/CachedData"),
        description: Cow::Borrowed("Editor cached extensions/data"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("VS Code ShipIt Cache"),
        path: Cow::Borrowed("~/Library/Caches/com.microsoft.VSCode.ShipIt"),
        description: Cow::Borrowed("VS Code updater downloads"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("iOS Simulators"),
        path: Cow::Borrowed("~/Library/Developer/CoreSimulator/Caches"),
        description: Cow::Borrowed("Simulator app caches"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("iOS Simulator Devices"),
        path: Cow::Borrowed("~/Library/Developer/CoreSimulator/Devices"),
        description: Cow::Borrowed("Installed simulator devices and their data"),
        command: &[],
        requires_sudo: false,
        dangerous: true,
        delete_entire: true,
    },
    CleanTarget {
        name: Cow::Borrowed("Xcode DerivedData"),
        path: Cow::Borrowed("~/Library/Developer/Xcode/DerivedData"),
        description: Cow::Borrowed("Xcode build artifacts"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("User Caches"),
        path: Cow::Borrowed("~/Library/Caches"),
        description: Cow::Borrowed("General application caches"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("User Logs"),
        path: Cow::Borrowed("~/Library/Logs"),
        description: Cow::Borrowed("Application crash logs and telemetry"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Trash"),
        path: Cow::Borrowed("~/.Trash"),
        description: Cow::Borrowed("User trash bin"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Homebrew Cache"),
        path: Cow::Borrowed("~/Library/Caches/Homebrew"),
        description: Cow::Borrowed("Formula downloads"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("iOS DeviceSupport"),
        path: Cow::Borrowed("~/Library/Developer/Xcode/iOS DeviceSupport"),
        description: Cow::Borrowed("Device debug symbols"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Firefox Cache"),
        path: Cow::Borrowed("~/Library/Caches/Firefox"),
        description: Cow::Borrowed("Browser cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Chrome Cache"),
        path: Cow::Borrowed("~/Library/Caches/Google/Chrome"),
        description: Cow::Borrowed("Google Chrome cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Chromium Cache"),
        path: Cow::Borrowed("~/Library/Caches/Chromium"),
        description: Cow::Borrowed("Chromium browser cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Brave Cache"),
        path: Cow::Borrowed("~/Library/Caches/BraveSoftware/Brave-Browser"),
        description: Cow::Borrowed("Brave browser cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Xcode Archives"),
        path: Cow::Borrowed("~/Library/Developer/Xcode/Archives"),
        description: Cow::Borrowed("Archived app builds"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Xcode Device Logs"),
        path: Cow::Borrowed("~/Library/Developer/Xcode/Devices"),
        description: Cow::Borrowed("Device crash logs and console data"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Xcode Previews Cache"),
        path: Cow::Borrowed("~/Library/Developer/Xcode/UserData/Previews"),
        description: Cow::Borrowed("SwiftUI preview compilation cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: true,
    },
    CleanTarget {
        name: Cow::Borrowed("Xcode Caches"),
        path: Cow::Borrowed("~/Library/Caches/com.apple.dt.Xcode"),
        description: Cow::Borrowed("SourceKit index and symbol caches"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: true,
    },
    CleanTarget {
        name: Cow::Borrowed("iOS Device Backups"),
        path: Cow::Borrowed("~/Library/Application Support/MobileSync/Backup"),
        description: Cow::Borrowed("iPhone/iPad encrypted backups"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("iOS Simulator Runtimes"),
        path: Cow::Borrowed("/Library/Developer/CoreSimulator/Images"),
        description: Cow::Borrowed("Simulator OS runtime images"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Time Machine Local Snapshots"),
        path: Cow::Borrowed(""),
        description: Cow::Borrowed("APFS local Time Machine snapshots (may require sudo)"),
        command: &["tmutil", "deletelocalsnapshots", "/"],
        requires_sudo: true,
        dangerous: true,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Docker System Prune"),
        path: Cow::Borrowed(""),
        description: Cow::Borrowed("Remove all unused Docker containers, images, and build cache"),
        command: &["docker", "system", "prune", "-a", "--force"],
        requires_sudo: false,
        dangerous: true,
        delete_entire: false,
    },
];

#[cfg(target_os = "linux")]
pub const OS_CACHES: &[CleanTarget] = &[
    CleanTarget {
        name: Cow::Borrowed("Go Build Cache"),
        path: Cow::Borrowed("~/.cache/go-build"),
        description: Cow::Borrowed("Go compiler build cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("pnpm Cache"),
        path: Cow::Borrowed("~/.cache/pnpm"),
        description: Cow::Borrowed("pnpm package store"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Yarn Cache"),
        path: Cow::Borrowed("~/.cache/yarn"),
        description: Cow::Borrowed("Yarn package cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("pip Cache"),
        path: Cow::Borrowed("~/.cache/pip"),
        description: Cow::Borrowed("Python package cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Playwright Browsers"),
        path: Cow::Borrowed("~/.cache/ms-playwright"),
        description: Cow::Borrowed("Playwright browser downloads"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("VS Code Cache"),
        path: Cow::Borrowed("~/.config/Code/Cache"),
        description: Cow::Borrowed("Editor cache files"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("VS Code CachedData"),
        path: Cow::Borrowed("~/.config/Code/CachedData"),
        description: Cow::Borrowed("Editor cached extensions/data"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("VS Code CachedExtensions"),
        path: Cow::Borrowed("~/.config/Code/CachedExtensions"),
        description: Cow::Borrowed("Editor extension cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("User Caches"),
        path: Cow::Borrowed("~/.cache"),
        description: Cow::Borrowed("Standard XDG cache directory"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Thumbnail Cache"),
        path: Cow::Borrowed("~/.cache/thumbnails"),
        description: Cow::Borrowed("Image explorer thumbnails"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Systemd Journal Logs"),
        path: Cow::Borrowed("/var/log/journal"),
        description: Cow::Borrowed("Systemd binary logs (Requires sudo)"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Apt Cache"),
        path: Cow::Borrowed("/var/cache/apt/archives"),
        description: Cow::Borrowed("Debian/Ubuntu package cache (Requires sudo)"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Trash"),
        path: Cow::Borrowed("~/.local/share/Trash"),
        description: Cow::Borrowed("XDG trash bin"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Pacman Cache"),
        path: Cow::Borrowed("/var/cache/pacman/pkg"),
        description: Cow::Borrowed("Arch package cache (Requires sudo)"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Yay Cache"),
        path: Cow::Borrowed("~/.cache/yay"),
        description: Cow::Borrowed("AUR helper build cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Paru Cache"),
        path: Cow::Borrowed("~/.cache/paru"),
        description: Cow::Borrowed("AUR helper build cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Flatpak Cache"),
        path: Cow::Borrowed("~/.cache/flatpak"),
        description: Cow::Borrowed("Flatpak app downloads"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Snap Cache"),
        path: Cow::Borrowed("/var/lib/snapd/cache"),
        description: Cow::Borrowed("Snap package cache (Requires sudo)"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Docker Overlay2"),
        path: Cow::Borrowed("/var/lib/docker/overlay2"),
        description: Cow::Borrowed("Docker container layers (Requires sudo)"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Firefox Cache"),
        path: Cow::Borrowed("~/.cache/mozilla/firefox"),
        description: Cow::Borrowed("Browser cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Chrome Cache"),
        path: Cow::Borrowed("~/.cache/google-chrome"),
        description: Cow::Borrowed("Google Chrome cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Chromium Cache"),
        path: Cow::Borrowed("~/.cache/chromium"),
        description: Cow::Borrowed("Chromium browser cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Brave Cache"),
        path: Cow::Borrowed("~/.cache/BraveSoftware/Brave-Browser"),
        description: Cow::Borrowed("Brave browser cache"),
        command: &[],
        requires_sudo: false,
        dangerous: false,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Docker System Prune"),
        path: Cow::Borrowed(""),
        description: Cow::Borrowed("Remove all unused Docker containers, images, and build cache"),
        command: &["docker", "system", "prune", "-a", "--force"],
        requires_sudo: false,
        dangerous: true,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Apt Autoremove"),
        path: Cow::Borrowed(""),
        description: Cow::Borrowed("Remove orphaned apt packages (Requires sudo)"),
        command: &["sudo", "apt", "autoremove", "-y"],
        requires_sudo: true,
        dangerous: true,
        delete_entire: false,
    },
    CleanTarget {
        name: Cow::Borrowed("Journalctl Vacuum"),
        path: Cow::Borrowed(""),
        description: Cow::Borrowed("Trim systemd journal logs to 100MB (Requires sudo)"),
        command: &["sudo", "journalctl", "--vacuum-size=100M"],
        requires_sudo: true,
        dangerous: false,
        delete_entire: false,
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
            command: &[],
            requires_sudo: false,
            dangerous: false,
            delete_entire: false,
        })
        .collect();

    let mut all =
        Vec::with_capacity(DEV_CACHES.len() + OS_CACHES.len() + EXTRA_CACHES.len() + custom.len());
    all.extend_from_slice(DEV_CACHES);
    all.extend(
        OS_CACHES
            .iter()
            .filter(|t| t.is_command() || t.resolved_path().exists())
            .cloned(),
    );
    all.extend(
        EXTRA_CACHES
            .iter()
            .filter(|t| t.is_command() || t.resolved_path().exists())
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
