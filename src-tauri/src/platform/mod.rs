//! Thin wrappers over the operating system.
//!
//! All `cfg(windows)` code lives under this module, so the rest of the backend stays portable and
//! testable on any machine. Process discovery and Windows-specific tests arrive in Phase 4.

pub mod roblox;

/// Operating system name, e.g. `"windows"`.
pub fn os() -> &'static str {
    std::env::consts::OS
}

/// CPU architecture, e.g. `"x86_64"`.
pub fn arch() -> &'static str {
    std::env::consts::ARCH
}

/// Number of logical processors available to this process, or `None` if the OS will not say.
pub fn logical_cores() -> Option<u32> {
    std::thread::available_parallelism()
        .ok()
        .and_then(|n| u32::try_from(n.get()).ok())
}

/// A snapshot of physical memory, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryStatus {
    pub total: u64,
    pub available: u64,
}

impl MemoryStatus {
    /// Bytes in use. Never underflows, even if the OS reports `available > total`.
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.available)
    }
}

/// Physical memory right now. Implemented for Windows only (`GlobalMemoryStatusEx`); on other
/// systems it returns `None` rather than an invented number.
pub fn memory_status() -> Option<MemoryStatus> {
    memory_status_impl()
}

#[cfg(windows)]
fn memory_status_impl() -> Option<MemoryStatus> {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    let mut m = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: `m` is a local with `dwLength` set, as GlobalMemoryStatusEx requires.
    unsafe { GlobalMemoryStatusEx(&mut m) }.ok()?;
    Some(MemoryStatus {
        total: m.ullTotalPhys,
        available: m.ullAvailPhys,
    })
}

#[cfg(not(windows))]
fn memory_status_impl() -> Option<MemoryStatus> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_this_machine() {
        assert_eq!(os(), std::env::consts::OS);
        assert_eq!(arch(), std::env::consts::ARCH);
    }

    #[test]
    fn has_at_least_one_logical_core() {
        assert!(logical_cores().is_some_and(|n| n >= 1));
    }

    #[test]
    fn used_memory_never_underflows() {
        let m = MemoryStatus {
            total: 8,
            available: 3,
        };
        assert_eq!(m.used(), 5);
        let odd = MemoryStatus {
            total: 4,
            available: 9,
        };
        assert_eq!(odd.used(), 0);
    }

    /// Only meaningful on Windows, where it calls the real API. Not run in the Linux build.
    #[cfg(windows)]
    #[test]
    fn windows_memory_status_is_plausible() {
        let m = memory_status().expect("GlobalMemoryStatusEx should succeed");
        assert!(m.total > 0);
        assert!(m.available <= m.total);
    }

    #[cfg(not(windows))]
    #[test]
    fn memory_is_unavailable_off_windows() {
        assert_eq!(memory_status(), None);
    }
}
