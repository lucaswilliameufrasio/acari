/// Disk usage overview for the primary data volume.
///
/// - On macOS the writable user data lives under `/System/Volumes/Data`.
/// - On Linux the root filesystem is `/`.
pub struct DiskOverview {
    pub device: String,
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub usage_percent: f64,
    pub purgeable: Option<u64>,
}

#[cfg(unix)]
use std::mem::MaybeUninit;

#[cfg(unix)]
fn statvfs(path: &str) -> Option<(u64, u64, u64)> {
    unsafe {
        let mut s: libc::statvfs = MaybeUninit::zeroed().assume_init();
        let cpath = std::ffi::CString::new(path).ok()?;
        if libc::statvfs(cpath.as_ptr(), &mut s) != 0 {
            return None;
        }
        let frsize = s.f_frsize as u128;
        let total = ((s.f_blocks as u128).saturating_mul(frsize)) as u64;
        let free = ((s.f_bavail as u128).saturating_mul(frsize)) as u64;
        let used = total.saturating_sub(((s.f_bfree as u128).saturating_mul(frsize)) as u64);
        Some((total, used, free))
    }
}

pub fn disk_overview() -> DiskOverview {
    #[cfg(target_os = "macos")]
    let path = "/System/Volumes/Data";
    #[cfg(target_os = "linux")]
    let path = "/";
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let path = "/";

    let device = path.to_string();

    #[cfg(unix)]
    let (total, used, free) = statvfs(path).unwrap_or((0, 0, 0));
    #[cfg(not(unix))]
    let (total, used, free) = (0, 0, 0);

    let usage_percent = if total > 0 {
        (used as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let purgeable = crate::infrastructure::exec::purgeable_bytes();

    DiskOverview {
        device,
        total,
        used,
        free,
        usage_percent,
        purgeable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_returns_zero_or_positive_on_any_platform() {
        let o = disk_overview();
        assert!(!o.device.is_empty());
        // Values may be 0 on unsupported platforms, but never inconsistent.
        assert!(o.total >= o.used);
    }
}
