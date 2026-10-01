pub mod cleaner;
pub mod df;
pub mod disk_scan;
pub mod distro;
pub mod du;
pub mod exec;
pub mod history;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub mod privileged;
pub mod scanner;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub use linux as platform;
#[cfg(target_os = "macos")]
pub use macos as platform;
