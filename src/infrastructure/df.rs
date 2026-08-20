/// Disk usage overview for the primary data volume.
///
/// - On macOS the writable user data lives under `/System/Volumes/Data`.
/// - On Linux the root filesystem is `/`.
pub struct DiskOverview {
    pub device: String,
    /// Whether the volume statistics could be read (false if unavailable).
    pub available: bool,
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

/// Windows equivalent of `statvfs`, via `GetDiskFreeSpaceExW`.
#[cfg(windows)]
fn disk_usage_windows() -> Option<(u64, u64, u64)> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetDiskFreeSpaceExW(
            lpDirectoryName: *const u16,
            lpFreeBytesAvailableToCaller: *mut u64,
            lpTotalNumberOfBytes: *mut u64,
            lpTotalNumberOfFreeBytes: *mut u64,
        ) -> i32;
    }

    let dir = std::env::current_dir().ok()?;
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();

    let mut free_available: u64 = 0;
    let mut total: u64 = 0;
    let mut free_total: u64 = 0;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_available,
            &mut total,
            &mut free_total,
        )
    };
    if ok == 0 {
        return None;
    }
    let used = total.saturating_sub(free_total);
    Some((total, used, free_available))
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
    let stats = statvfs(path);
    #[cfg(windows)]
    let stats = disk_usage_windows();
    #[cfg(not(any(unix, windows)))]
    let stats: Option<(u64, u64, u64)> = None;

    let (available, total, used, free) = match stats {
        Some((total, used, free)) => (true, total, used, free),
        None => (false, 0, 0, 0),
    };

    let usage_percent = if total > 0 {
        (used as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let purgeable = crate::infrastructure::exec::purgeable_bytes();

    DiskOverview {
        device,
        available,
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
