#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use self::windows::{list_devices, Device, Subscription};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use self::macos::{list_devices, Device, Subscription};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use self::linux::{list_devices, Device, Subscription};

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
mod unsupported;
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub use self::unsupported::{list_devices, Device, Subscription};
