pub mod cleaner;
pub mod distro;
pub mod history;
pub mod scanner;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub use linux as platform;
#[cfg(target_os = "macos")]
pub use macos as platform;
