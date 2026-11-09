pub mod agents;
pub mod cleaner;
pub mod df;
pub mod disk_scan;
pub mod distro;
pub mod du;
pub mod exec;
pub mod explore;
pub mod history;
pub mod notify;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub mod privileged;
pub mod scanner;
pub mod state;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub use linux as platform;
#[cfg(target_os = "macos")]
pub use macos as platform;
