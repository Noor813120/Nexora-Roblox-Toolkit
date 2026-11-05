//! Thin wrappers over the operating system.
//!
//! All `cfg(windows)` code lives under this module, so the rest of the backend stays portable and
//! testable on any machine. Process discovery and Windows-specific tests arrive in Phase 4.

/// Operating system name, e.g. `"windows"`.
pub fn os() -> &'static str {
    std::env::consts::OS
}

/// CPU architecture, e.g. `"x86_64"`.
pub fn arch() -> &'static str {
    std::env::consts::ARCH
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_this_machine() {
        assert_eq!(os(), std::env::consts::OS);
        assert_eq!(arch(), std::env::consts::ARCH);
    }
}
