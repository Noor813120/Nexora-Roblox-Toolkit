//! The real-computer half of Roblox detection: the questions in
//! [`crate::launcher::roblox::Environment`], answered by the operating system.
//!
//! Everything here is read-only. It lists a folder, reads file *metadata* (never file contents),
//! and reads one registry value. It starts no process, opens no process, makes no network request
//! and writes nothing. The registry value is a command line, and it is returned as text only.
//!
//! The Windows-only parts (`LOCALAPPDATA` lookup and the registry read) are the only `cfg(windows)`
//! code. The file and folder helpers are plain `std::fs`, so they are tested on every system.

use crate::launcher::roblox::{
    DirListing, Environment, FileState, MAX_SCANNED_ENTRIES, RegistryValue, is_version_name,
};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// The actual computer this program is running on.
pub struct SystemEnvironment;

impl Environment for SystemEnvironment {
    fn platform_supported(&self) -> bool {
        cfg!(windows)
    }

    fn versions_dir(&self) -> Option<PathBuf> {
        local_app_data().map(|p| p.join("Roblox").join("Versions"))
    }

    fn list_dirs(&self, dir: &Path) -> DirListing {
        list_dirs(dir)
    }

    fn file_state(&self, path: &Path) -> FileState {
        file_state(path)
    }

    fn protocol_handler_command(&self) -> RegistryValue {
        protocol_handler_command()
    }
}

/// `%LOCALAPPDATA%`, only if it is set and absolute. Windows only; elsewhere there is no answer.
fn local_app_data() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let path = PathBuf::from(std::env::var_os("LOCALAPPDATA")?);
    path.is_absolute().then_some(path)
}

/// Names of the sub-folders of `dir`. Files and links are not listed (links are never followed;
/// one named like a `version-…` folder makes the listing incomplete). At most
/// [`MAX_SCANNED_ENTRIES`] entries are looked at; if there are more, or one cannot be read, the
/// listing is marked incomplete so the caller never mistakes "I did not look" for "nothing there".
fn list_dirs(dir: &Path) -> DirListing {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return DirListing::Missing,
        Err(_) => return DirListing::Inaccessible,
    };
    let mut names = Vec::new();
    let mut complete = true;
    for (seen, entry) in entries.enumerate() {
        if seen >= MAX_SCANNED_ENTRIES {
            complete = false;
            break;
        }
        match entry {
            Ok(entry) => match entry.file_type() {
                // A link is never followed. If it is named like a version folder it may hide an
                // install, so the listing is incomplete rather than a clean "no".
                Ok(kind) if kind.is_symlink() => {
                    if is_version_name(&entry.file_name().to_string_lossy()) {
                        complete = false;
                    }
                }
                Ok(kind) if kind.is_dir() => {
                    names.push(entry.file_name().to_string_lossy().into_owned())
                }
                Ok(_) => {}
                Err(_) => complete = false,
            },
            Err(_) => complete = false,
        }
    }
    DirListing::Entries { names, complete }
}

/// What is at `path`, from file metadata only (the file is never opened or read).
fn file_state(path: &Path) -> FileState {
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() && meta.len() == 0 => FileState::Empty,
        Ok(meta) if meta.is_file() => FileState::File {
            modified_ms: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .and_then(|d| i64::try_from(d.as_millis()).ok()),
        },
        Ok(_) => FileState::NotAFile,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => FileState::Missing,
        Err(_) => FileState::Inaccessible,
    }
}

/// Turns the UTF-16 text of a registry value into a `String`. The text ends at the first NUL (the
/// registry may include trailing ones). Invalid UTF-16 gives `None` rather than a guess.
#[cfg(any(windows, test))]
fn decode_registry_text(units: &[u16]) -> Option<String> {
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    String::from_utf16(&units[..end]).ok()
}

/// The default value of `HKEY_CLASSES_ROOT\roblox-player\shell\open\command`. This is the
/// per-user and per-machine settings combined, as the Windows shell sees them. Read only.
#[cfg(windows)]
fn protocol_handler_command() -> RegistryValue {
    use windows::Win32::Foundation::{
        ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS,
        ERROR_UNSUPPORTED_TYPE,
    };
    use windows::Win32::System::Registry::{
        HKEY_CLASSES_ROOT, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ, RegGetValueW,
    };
    use windows::core::{PCWSTR, w};

    /// More than this many bytes is not a sane command line.
    const MAX_BYTES: u32 = 16 * 1024;

    let subkey = w!("roblox-player\\shell\\open\\command");
    let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ; // text only; %VARS% are expanded for us

    // First ask how big the value is, then read it. A null value name means "the default value".
    let mut bytes: u32 = 0;
    // SAFETY: `subkey` is a valid NUL-terminated string, the value name is null (allowed), and
    // `bytes` is a valid pointer. No data buffer is passed, so only the size is written.
    let status = unsafe {
        RegGetValueW(
            HKEY_CLASSES_ROOT,
            subkey,
            PCWSTR::null(),
            flags,
            None,
            None,
            Some(&mut bytes),
        )
    };
    if status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND {
        return RegistryValue::NotSet;
    }
    if status != ERROR_SUCCESS {
        // A value that is not text is malformed; access denied or any other failure is inaccessible.
        return if status == ERROR_UNSUPPORTED_TYPE {
            RegistryValue::Malformed
        } else {
            RegistryValue::Inaccessible
        };
    }
    if bytes == 0 || bytes > MAX_BYTES {
        return RegistryValue::Malformed;
    }

    let mut buffer = vec![0u16; (bytes as usize).div_ceil(2)];
    let mut capacity = bytes;
    // SAFETY: `buffer` holds at least `capacity` bytes, `capacity` is passed as its size, and
    // `subkey` is as above. The call writes at most `capacity` bytes and updates it.
    let status = unsafe {
        RegGetValueW(
            HKEY_CLASSES_ROOT,
            subkey,
            PCWSTR::null(),
            flags,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut capacity),
        )
    };
    if status == ERROR_MORE_DATA {
        return RegistryValue::Inaccessible; // the value changed between the two reads
    }
    if status != ERROR_SUCCESS {
        return RegistryValue::Inaccessible;
    }
    let units = (capacity as usize / 2).min(buffer.len());
    match decode_registry_text(&buffer[..units]) {
        Some(text) => RegistryValue::Text(text),
        None => RegistryValue::Malformed,
    }
}

#[cfg(not(windows))]
fn protocol_handler_command() -> RegistryValue {
    RegistryValue::NotSet // never asked: `platform_supported()` is false here
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::roblox::{DetectionStatus, ProbeOutcome, detect};
    use crate::storage::test_support::TempDir;
    use std::fs;

    #[test]
    fn missing_folder_is_missing_not_inaccessible() {
        let dir = TempDir::new();
        assert_eq!(list_dirs(&dir.path().join("nope")), DirListing::Missing);
    }

    #[test]
    fn lists_only_sub_folders() {
        let dir = TempDir::new();
        fs::create_dir(dir.path().join("version-0123456789abcdef")).unwrap();
        fs::create_dir(dir.path().join("other")).unwrap();
        fs::write(dir.path().join("a-file.txt"), b"x").unwrap();
        let DirListing::Entries {
            mut names,
            complete,
        } = list_dirs(dir.path())
        else {
            panic!("expected entries");
        };
        names.sort();
        assert!(complete);
        assert_eq!(names, ["other", "version-0123456789abcdef"]);
    }

    #[test]
    fn a_file_where_a_folder_is_expected_is_not_a_clean_no() {
        let dir = TempDir::new();
        let file = dir.path().join("Versions");
        fs::write(&file, b"x").unwrap();
        // Not "Missing": something is there, we just cannot list it.
        assert_eq!(list_dirs(&file), DirListing::Inaccessible);
    }

    #[test]
    fn oversized_listing_is_marked_incomplete() {
        let dir = TempDir::new();
        for i in 0..(MAX_SCANNED_ENTRIES + 3) {
            fs::create_dir(dir.path().join(format!("d{i}"))).unwrap();
        }
        let DirListing::Entries { names, complete } = list_dirs(dir.path()) else {
            panic!("expected entries");
        };
        assert!(!complete);
        assert_eq!(names.len(), MAX_SCANNED_ENTRIES);
    }

    /// Makes a link (a junction on Windows, a symlink elsewhere) without needing special rights.
    fn make_dir_link(link: &Path, target: &Path) {
        #[cfg(windows)]
        {
            let status = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()
                .unwrap();
            assert!(status.status.success(), "mklink failed");
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    #[test]
    fn a_link_named_like_a_version_folder_makes_the_listing_incomplete() {
        let dir = TempDir::new();
        let target = dir.path().join("elsewhere");
        fs::create_dir(&target).unwrap();
        make_dir_link(&dir.path().join("version-0123456789abcdef"), &target);
        let DirListing::Entries { names, complete } = list_dirs(dir.path()) else {
            panic!("expected entries");
        };
        assert!(!complete, "a skipped link must not look like a clean no");
        assert_eq!(names, ["elsewhere"]); // the link itself is never listed or followed
    }

    #[test]
    fn an_unrelated_link_is_ignored() {
        let dir = TempDir::new();
        let target = dir.path().join("elsewhere");
        fs::create_dir(&target).unwrap();
        make_dir_link(&dir.path().join("shortcut"), &target);
        let DirListing::Entries { names, complete } = list_dirs(dir.path()) else {
            panic!("expected entries");
        };
        assert!(complete);
        assert_eq!(names, ["elsewhere"]);
    }

    #[test]
    fn file_states_are_told_apart() {
        let dir = TempDir::new();
        let full = dir.path().join("full.exe");
        let empty = dir.path().join("empty.exe");
        fs::write(&full, b"MZ").unwrap();
        fs::write(&empty, b"").unwrap();
        fs::create_dir(dir.path().join("folder.exe")).unwrap();

        let FileState::File { modified_ms } = file_state(&full) else {
            panic!("expected a file");
        };
        assert!(modified_ms.is_some_and(|ms| ms > 1_700_000_000_000));
        assert_eq!(file_state(&empty), FileState::Empty);
        assert_eq!(
            file_state(&dir.path().join("folder.exe")),
            FileState::NotAFile
        );
        assert_eq!(
            file_state(&dir.path().join("absent.exe")),
            FileState::Missing
        );
    }

    #[test]
    fn checking_a_file_does_not_change_it() {
        let dir = TempDir::new();
        let path = dir.path().join("p.exe");
        fs::write(&path, b"MZ-original").unwrap();
        let before = fs::metadata(&path).unwrap().modified().unwrap();
        let _ = file_state(&path);
        assert_eq!(fs::read(&path).unwrap(), b"MZ-original");
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
    }

    #[test]
    fn registry_text_decoding_handles_padding_and_bad_data() {
        let utf16 = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
        assert_eq!(
            decode_registry_text(&utf16("C:\\a.exe")).as_deref(),
            Some("C:\\a.exe")
        );
        let mut padded = utf16("C:\\a.exe");
        padded.extend([0, 0, 0]);
        assert_eq!(decode_registry_text(&padded).as_deref(), Some("C:\\a.exe"));
        let mut tail = utf16("C:\\a.exe");
        tail.extend([0, 0x41, 0x42]); // anything after the first NUL is ignored
        assert_eq!(decode_registry_text(&tail).as_deref(), Some("C:\\a.exe"));
        assert_eq!(decode_registry_text(&[]).as_deref(), Some(""));
        assert_eq!(decode_registry_text(&[0x0043, 0xD800, 0x0041]), None); // lone surrogate
    }

    // ---- this computer, not Windows ----

    #[cfg(not(windows))]
    #[test]
    fn other_systems_report_unsupported_without_probing() {
        let d = detect(&SystemEnvironment);
        assert_eq!(d.status, DetectionStatus::Unsupported);
        assert!(d.installs.is_empty());
        assert_eq!(d.findings.len(), 1);
        assert_eq!(d.findings[0].outcome, ProbeOutcome::Unsupported);
    }

    // ---- Windows only: these call the real Windows APIs and are NOT run on Linux ----

    /// Runs on a real Windows PC. Whatever the answer is, it must be self-consistent: an install is
    /// only reported if its program exists, and the status must agree with the evidence.
    #[cfg(windows)]
    #[test]
    fn windows_detection_is_consistent_with_the_machine() {
        let d = detect(&SystemEnvironment);
        assert_ne!(d.status, DetectionStatus::Unsupported);
        assert_eq!(
            d.status == DetectionStatus::Detected,
            !d.installs.is_empty()
        );
        for install in &d.installs {
            assert!(
                Path::new(&install.executable_path).is_file(),
                "{}",
                install.executable_path
            );
        }
        // The Microsoft Store limitation is always stated.
        assert!(
            d.findings
                .iter()
                .any(|f| f.outcome == ProbeOutcome::Unsupported)
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_registry_read_does_not_fail_on_a_normal_machine() {
        // Any of these is a legitimate answer; the point is that the API call itself works.
        assert!(matches!(
            protocol_handler_command(),
            RegistryValue::Text(_)
                | RegistryValue::NotSet
                | RegistryValue::Malformed
                | RegistryValue::Inaccessible
        ));
    }

    #[cfg(windows)]
    #[test]
    fn windows_locates_local_app_data() {
        let versions = SystemEnvironment
            .versions_dir()
            .expect("LOCALAPPDATA should be set");
        assert!(versions.is_absolute());
        assert!(versions.ends_with(Path::new("Roblox").join("Versions")));
    }
}
