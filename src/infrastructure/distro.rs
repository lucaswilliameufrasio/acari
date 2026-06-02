use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct DistroInfo {
    pub id: String,
    pub pretty_name: String,
}

pub fn detect() -> DistroInfo {
    cfg_if_detect()
}

#[cfg(target_os = "linux")]
fn cfg_if_detect() -> DistroInfo {
    let os_release_path = Path::new("/etc/os-release");
    if !os_release_path.exists() {
        return DistroInfo {
            id: String::from("linux"),
            pretty_name: String::from("Linux"),
        };
    }

    let content = match fs::read_to_string(os_release_path) {
        Ok(c) => c,
        Err(_) => {
            return DistroInfo {
                id: String::from("linux"),
                pretty_name: String::from("Linux"),
            };
        }
    };

    let mut fields: HashMap<String, String> = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let val = value.trim().trim_matches('"').to_owned();
            fields.insert(key.to_owned(), val);
        }
    }

    DistroInfo {
        id: fields
            .get("ID")
            .cloned()
            .unwrap_or_else(|| String::from("linux")),
        pretty_name: fields
            .get("PRETTY_NAME")
            .cloned()
            .unwrap_or_else(|| String::from("Linux")),
    }
}

#[cfg(target_os = "macos")]
fn cfg_if_detect() -> DistroInfo {
    DistroInfo {
        id: String::from("macos"),
        pretty_name: String::from("macOS"),
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn cfg_if_detect() -> DistroInfo {
    DistroInfo {
        id: String::from("unknown"),
        pretty_name: String::from("Unknown OS"),
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    #[test]
    fn detect_returns_something_on_linux() {
        let info = super::detect();
        assert!(!info.id.is_empty());
        assert!(!info.pretty_name.is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn detect_returns_macos() {
        let info = super::detect();
        assert_eq!(info.id, "macos");
        assert_eq!(info.pretty_name, "macOS");
    }
}
