//! What the installer does to the computer. Shared types will move here as Linux support is
//! added (ops/linux.rs); for now everything lives in the Windows module.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;
