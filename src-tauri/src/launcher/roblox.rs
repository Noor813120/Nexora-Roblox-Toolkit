//! Read-only detection of an installed Roblox player (Phase 4, Milestone 1).
//!
//! This module is the *thinking* half of detection. It never touches the computer itself: it asks
//! an [`Environment`] (folders, files, one registry value) and decides what the answers mean. The
//! real computer is wrapped in `platform::roblox`; the tests here use a made-up one. That keeps the
//! rules testable on any machine, and keeps every `cfg(windows)` line out of this file.
//!
//! What detection will and will not do:
//! - It only reads: it lists a folder, checks that a file exists, and reads one registry value.
//! - It never runs anything. The registry value is a *command line*; it is split into a program
//!   path and ignored arguments, and only the path is looked at (is there a file there?).
//! - It reports an install only after the expected program file exists and is a non-empty file.
//!   A folder name alone is never enough, and neither is a registry value. File contents are
//!   never read, so an install has only passed basic checks; it is not proof of genuine Roblox.
//! - A path it cannot read is "unknown", never "not installed".
//! - It does not claim to support Microsoft Store installs; it says so in the result.

use serde::Serialize;
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// The game client inside a version folder.
pub const PLAYER_EXE: &str = "RobloxPlayerBeta.exe";
/// Programs the `roblox-player` protocol handler is expected to point to (lower case).
const HANDLER_EXES: [&str; 2] = ["robloxplayerlauncher.exe", "robloxplayerbeta.exe"];
/// At most this many entries of the `Versions` folder are examined; more is reported as incomplete.
pub const MAX_SCANNED_ENTRIES: usize = 1024;
/// A registry command line longer than this is treated as malformed rather than parsed.
const MAX_COMMAND_LEN: usize = 4096;
/// Individual problem lines shown per probe before the rest are summarised as a count.
const MAX_REPORTED_PROBLEMS: usize = 10;

// ---------------------------------------------------------------------------------------------
// Result types (sent to the frontend)
// ---------------------------------------------------------------------------------------------

/// The overall answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DetectionStatus {
    /// At least one program file passed the basic checks (expected path, regular file, size above
    /// zero). Its contents were not checked, so this is not proof of genuine Roblox software.
    Detected,
    /// Every supported location was checked and none held an install. Microsoft Store installs
    /// are not checked.
    NotDetected,
    /// Nothing was found, but something could not be checked, so Roblox may still be installed.
    Inconclusive,
    /// Detection does not run on this operating system.
    Unsupported,
}

/// Where a piece of evidence came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DetectionSource {
    /// `%LOCALAPPDATA%\Roblox\Versions\version-…\RobloxPlayerBeta.exe`.
    VersionFolder,
    /// The Windows `roblox-player` URL-protocol handler.
    ProtocolHandler,
    /// A Microsoft Store install. Not checked; see [`detect`].
    MicrosoftStore,
    /// The operating system itself.
    Platform,
}

/// What one check found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ProbeOutcome {
    /// A program file passed the basic checks. Its contents were not checked.
    Found,
    /// The place looked at does not exist (a clean "no").
    NotFound,
    /// A folder or registry entry exists, but the program file it should contain does not.
    MissingExecutable,
    /// The program path exists but is not a regular, non-empty file.
    NotAFile,
    /// The place could not be read (permissions, locked, unknown location). This is NOT a "no".
    Inaccessible,
    /// Part of the place could not be examined (too many entries). Also NOT a "no".
    Incomplete,
    /// A value or name was present but did not make sense, so it was ignored.
    Malformed,
    /// The registry points to a program that is not a Roblox player.
    Unrecognised,
    /// This check is not supported (other operating system, or Microsoft Store installs).
    Unsupported,
    /// A summary of several problems of possibly different kinds; the detail says which.
    Several,
}

/// One install whose program file passed the basic checks (contents not checked).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RobloxInstall {
    pub source: DetectionSource,
    /// The `version-…` folder name when the program lives in one.
    pub version: Option<String>,
    /// Full path of the program file.
    pub executable_path: String,
    /// When the program file was last modified (ms since 1970), if the system said.
    pub modified_at: Option<i64>,
}

/// One line of evidence, whether it helped or not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Finding {
    pub source: DetectionSource,
    pub outcome: ProbeOutcome,
    /// A short plain-language sentence for the screen. Never contains command-line arguments.
    pub detail: String,
}

/// Everything detection learned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RobloxDetection {
    pub status: DetectionStatus,
    /// Installs that passed the basic checks. The first is the one Windows would open (protocol handler) or, failing
    /// that, the most recently updated version folder. Empty unless `status` is `detected`.
    pub installs: Vec<RobloxInstall>,
    pub findings: Vec<Finding>,
}

// ---------------------------------------------------------------------------------------------
// What detection needs to ask the computer
// ---------------------------------------------------------------------------------------------

/// Result of looking at one path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    /// A regular file with content.
    File {
        modified_ms: Option<i64>,
    },
    /// A regular file with no content.
    Empty,
    Missing,
    /// Something exists there, but it is a folder or another kind of entry.
    NotAFile,
    Inaccessible,
}

/// Result of listing the sub-folders of one folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirListing {
    /// Folder names only. `complete` is false when the listing was cut short or an entry failed.
    Entries {
        names: Vec<String>,
        complete: bool,
    },
    Missing,
    Inaccessible,
}

/// Result of reading the protocol handler's command line.
///
/// Only the Windows registry code builds `Text`, `Inaccessible` and `Malformed`, so on other
/// systems the compiler would call them unused. The detection rules (and their tests) use them.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum RegistryValue {
    Text(String),
    NotSet,
    Inaccessible,
    /// Present but of the wrong type, too large, or not valid text.
    Malformed,
}

/// The questions detection asks. Every method is read-only; none can run a program.
pub trait Environment {
    fn platform_supported(&self) -> bool;
    /// `%LOCALAPPDATA%\Roblox\Versions`, or `None` if the local app-data folder is unknown.
    fn versions_dir(&self) -> Option<PathBuf>;
    fn list_dirs(&self, dir: &Path) -> DirListing;
    fn file_state(&self, path: &Path) -> FileState;
    /// The default value of `roblox-player\shell\open\command`.
    fn protocol_handler_command(&self) -> RegistryValue;
}

// ---------------------------------------------------------------------------------------------
// Parsing (pure, never touches the computer)
// ---------------------------------------------------------------------------------------------

/// Why a registry command line was not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    Empty,
    TooLong,
    InvalidCharacter,
    UnterminatedQuote,
    /// Text directly after the closing quote, such as `"C:\a.exe"x`.
    UnexpectedText,
    /// No program ending in `.exe` could be found.
    NoExecutable,
    /// The program is not a plain local drive path such as `C:\folder\file.exe`.
    NotLocalPath,
    /// The path contains characters Windows does not allow, or a `..` step.
    UnsafePath,
}

impl CommandError {
    pub fn describe(self) -> &'static str {
        match self {
            Self::Empty => "The registered command is empty.",
            Self::TooLong => "The registered command is unreasonably long, so it was ignored.",
            Self::InvalidCharacter => {
                "The registered command contains control characters, so it was ignored."
            }
            Self::UnterminatedQuote => {
                "The registered command has an opening quote but no closing quote."
            }
            Self::UnexpectedText => {
                "The registered command has unexpected text right after the closing quote."
            }
            Self::NoExecutable => "The registered command does not name a program ending in .exe.",
            Self::NotLocalPath => {
                "The registered program is not a plain local path such as C:\\folder\\program.exe."
            }
            Self::UnsafePath => {
                "The registered program path contains characters or steps that are not allowed."
            }
        }
    }
}

/// Extracts the program path from a registry command line such as
/// `"C:\Users\me\AppData\Local\Roblox\Versions\version-abc\RobloxPlayerLauncher.exe" %1`.
///
/// Only the program is returned; arguments are discarded and never interpreted. Quoted programs
/// end at the closing quote. Unquoted ones end at the first `.exe` that is followed by a space or
/// the end of the text, so unquoted paths containing spaces still work. Anything that is not an
/// absolute local drive path to an `.exe` is rejected, which also rules out network (`\\server`)
/// paths: checking one could make the computer contact another machine.
pub fn parse_command(raw: &str) -> Result<String, CommandError> {
    if raw.len() > MAX_COMMAND_LEN {
        return Err(CommandError::TooLong);
    }
    if raw.chars().any(|c| c.is_control() && c != '\t') {
        return Err(CommandError::InvalidCharacter);
    }
    let text = raw.trim();
    if text.is_empty() {
        return Err(CommandError::Empty);
    }

    let program = if let Some(rest) = text.strip_prefix('"') {
        let end = rest.find('"').ok_or(CommandError::UnterminatedQuote)?;
        let after = &rest[end + 1..];
        if !after.is_empty() && !after.starts_with(char::is_whitespace) {
            return Err(CommandError::UnexpectedText);
        }
        &rest[..end]
    } else {
        unquoted_program(text).ok_or(CommandError::NoExecutable)?
    };
    validate_program_path(program)?;
    Ok(program.to_owned())
}

/// The text up to and including the first `.exe` that ends a word.
fn unquoted_program(text: &str) -> Option<&str> {
    let lower = text.to_ascii_lowercase(); // same byte offsets as `text`
    let mut from = 0;
    while let Some(found) = lower[from..].find(".exe") {
        let end = from + found + 4;
        if end == text.len() || text[end..].starts_with(char::is_whitespace) {
            return Some(&text[..end]);
        }
        from = end;
    }
    None
}

fn validate_program_path(path: &str) -> Result<(), CommandError> {
    if path.is_empty() {
        return Err(CommandError::Empty);
    }
    if !path.to_ascii_lowercase().ends_with(".exe") {
        return Err(CommandError::NoExecutable);
    }
    // Drive letter, colon, slash: `C:\…`. Anything else (relative, `\\server`, `\\?\`) is refused.
    let b = path.as_bytes();
    let local = b.len() >= 3
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && (b[2] == b'\\' || b[2] == b'/');
    if !local {
        return Err(CommandError::NotLocalPath);
    }
    let rest = &path[2..];
    if rest.contains([':', '<', '>', '"', '|', '?', '*']) {
        return Err(CommandError::UnsafePath);
    }
    let mut parts = rest.split(['\\', '/']);
    if parts.any(|p| p == "..") {
        return Err(CommandError::UnsafePath);
    }
    let file = rest.rsplit(['\\', '/']).next().unwrap_or("");
    if file.len() <= ".exe".len() {
        return Err(CommandError::NoExecutable); // a file literally named ".exe"
    }
    Ok(())
}

/// True for names like `version-0123456789abcdef`: the prefix, then 8 to 32 hex digits.
pub fn is_version_name(name: &str) -> bool {
    let b = name.as_bytes();
    (16..=40).contains(&b.len())
        && b[..8].eq_ignore_ascii_case(b"version-")
        && b[8..].iter().all(u8::is_ascii_hexdigit)
}

/// The `version-…` folder that directly contains the program, if the path has one.
pub fn version_from_path(program: &str) -> Option<String> {
    let mut parts = program.rsplit(['\\', '/']);
    parts.next()?; // the file name
    let folder = parts.next()?;
    is_version_name(folder).then(|| folder.to_owned())
}

fn file_name(program: &str) -> &str {
    program.rsplit(['\\', '/']).next().unwrap_or(program)
}

// ---------------------------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------------------------

fn finding(source: DetectionSource, outcome: ProbeOutcome, detail: impl Into<String>) -> Finding {
    Finding {
        source,
        outcome,
        detail: detail.into(),
    }
}

/// Runs every check and combines the evidence. Read-only; see the module comment.
///
/// Microsoft Store installs are **not** supported: they live in a protected folder this code has
/// not been verified against, so the result always says so rather than guessing.
pub fn detect(env: &dyn Environment) -> RobloxDetection {
    if !env.platform_supported() {
        return RobloxDetection {
            status: DetectionStatus::Unsupported,
            installs: Vec::new(),
            findings: vec![finding(
                DetectionSource::Platform,
                ProbeOutcome::Unsupported,
                "Roblox detection is only available on Windows.",
            )],
        };
    }

    let mut installs = Vec::new();
    let mut findings = Vec::new();
    // Each probe says whether it left something unresolved (could not read it, or could not make
    // sense of it). One source's clean "no" never cancels another source's "unclear".
    let handler_unresolved = probe_protocol_handler(env, &mut installs, &mut findings);
    let folders_unresolved = probe_version_folders(env, &mut installs, &mut findings);
    findings.push(finding(
        DetectionSource::MicrosoftStore,
        ProbeOutcome::Unsupported,
        "Microsoft Store installs are not checked by this version, so one could exist even if none was found here.",
    ));

    let status = if !installs.is_empty() {
        DetectionStatus::Detected
    } else if handler_unresolved || folders_unresolved {
        DetectionStatus::Inconclusive
    } else {
        DetectionStatus::NotDetected
    };
    RobloxDetection {
        status,
        installs,
        findings,
    }
}

/// Text-only comparison: ASCII case and `/` versus `\` are ignored. No file is touched and no
/// path is resolved, so this stays read-only.
fn same_path(a: &str, b: &str) -> bool {
    let norm = |c: u8| {
        if c == b'/' {
            b'\\'
        } else {
            c.to_ascii_lowercase()
        }
    };
    a.len() == b.len() && a.bytes().zip(b.bytes()).all(|(x, y)| norm(x) == norm(y))
}

/// Checks the `roblox-player` link handler. Returns true when the answer is unresolved: the value
/// could not be read, could not be understood, or names a program that could not be checked.
fn probe_protocol_handler(
    env: &dyn Environment,
    installs: &mut Vec<RobloxInstall>,
    findings: &mut Vec<Finding>,
) -> bool {
    use DetectionSource::ProtocolHandler as Src;
    let command = match env.protocol_handler_command() {
        RegistryValue::Text(t) => t,
        RegistryValue::NotSet => {
            findings.push(finding(
                Src,
                ProbeOutcome::NotFound,
                "Windows has no 'roblox-player' link handler registered.",
            ));
            return false;
        }
        RegistryValue::Inaccessible => {
            findings.push(finding(
                Src,
                ProbeOutcome::Inaccessible,
                "The 'roblox-player' link handler could not be read.",
            ));
            return true;
        }
        RegistryValue::Malformed => {
            // Something is registered, but it could not be understood: that is not a "no".
            findings.push(finding(Src, ProbeOutcome::Malformed, "The 'roblox-player' link handler has an unexpected type or size, so it could not be used."));
            return true;
        }
    };
    let program = match parse_command(&command) {
        Ok(p) => p,
        Err(e) => {
            findings.push(finding(Src, ProbeOutcome::Malformed, e.describe()));
            return true;
        }
    };
    let name = file_name(&program).to_ascii_lowercase();
    if !HANDLER_EXES.contains(&name.as_str()) {
        findings.push(finding(
            Src,
            ProbeOutcome::Unrecognised,
            format!("The 'roblox-player' link handler opens a different program ({}), so it was not counted as Roblox.", file_name(&program)),
        ));
        return false;
    }
    match env.file_state(Path::new(&program)) {
        FileState::File { modified_ms } => {
            installs.push(RobloxInstall {
                source: Src,
                version: version_from_path(&program),
                executable_path: program,
                modified_at: modified_ms,
            });
            findings.push(finding(
                Src,
                ProbeOutcome::Found,
                "The 'roblox-player' link handler points to an existing, non-empty program file. Contents were not checked.",
            ));
        }
        FileState::Missing => findings.push(finding(
            Src,
            ProbeOutcome::MissingExecutable,
            format!("The 'roblox-player' link handler points to {program}, which does not exist."),
        )),
        FileState::Empty => findings.push(finding(
            Src,
            ProbeOutcome::NotAFile,
            format!("{program} exists but is empty."),
        )),
        FileState::NotAFile => findings.push(finding(
            Src,
            ProbeOutcome::NotAFile,
            format!("{program} is not a regular file."),
        )),
        FileState::Inaccessible => {
            findings.push(finding(
                Src,
                ProbeOutcome::Inaccessible,
                format!("{program} could not be checked."),
            ));
            return true;
        }
    }
    false
}

/// Checks `%LOCALAPPDATA%\Roblox\Versions`. Returns true when the answer is unresolved: the
/// folder could not be located or read, the listing was incomplete, or any candidate's program
/// could not be checked (including candidates hidden by the findings cap). Folder names that are
/// not Roblox version names are ignored and do not make the answer unresolved.
fn probe_version_folders(
    env: &dyn Environment,
    installs: &mut Vec<RobloxInstall>,
    findings: &mut Vec<Finding>,
) -> bool {
    use DetectionSource::VersionFolder as Src;
    let Some(dir) = env.versions_dir() else {
        findings.push(finding(Src, ProbeOutcome::Inaccessible, "The local app-data folder could not be located, so the Roblox Versions folder was not checked."));
        return true;
    };
    let (names, complete) = match env.list_dirs(&dir) {
        DirListing::Missing => {
            findings.push(finding(
                Src,
                ProbeOutcome::NotFound,
                format!("There is no Roblox Versions folder at {}.", dir.display()),
            ));
            return false;
        }
        DirListing::Inaccessible => {
            findings.push(finding(
                Src,
                ProbeOutcome::Inaccessible,
                format!("The folder {} could not be read.", dir.display()),
            ));
            return true;
        }
        DirListing::Entries { names, complete } => (names, complete),
    };

    let (candidates, ignored): (Vec<String>, Vec<String>) =
        names.into_iter().partition(|n| is_version_name(n));
    if !ignored.is_empty() {
        findings.push(finding(
            Src,
            ProbeOutcome::Malformed,
            format!("{} folder(s) in the Versions folder were ignored because their names are not Roblox version names.", ignored.len()),
        ));
    }
    if !complete {
        findings.push(finding(
            Src,
            ProbeOutcome::Incomplete,
            format!("The Versions folder was too large or partly unreadable; only part of it (up to {MAX_SCANNED_ENTRIES} entries) was checked."),
        ));
    }

    // Check each candidate by looking for the program itself. The folder name's sort order says
    // nothing about which version is newest, so it is never used to choose.
    let mut verified: Vec<(String, PathBuf, Option<i64>)> = Vec::new();
    let mut problems: Vec<(ProbeOutcome, String)> = Vec::new();
    for name in &candidates {
        let exe = dir.join(name).join(PLAYER_EXE);
        match env.file_state(&exe) {
            FileState::File { modified_ms } => verified.push((name.clone(), exe, modified_ms)),
            FileState::Missing => problems.push((
                ProbeOutcome::MissingExecutable,
                format!("{name} has no {PLAYER_EXE}."),
            )),
            FileState::Empty => problems.push((
                ProbeOutcome::NotAFile,
                format!("{name}\\{PLAYER_EXE} is empty."),
            )),
            FileState::NotAFile => problems.push((
                ProbeOutcome::NotAFile,
                format!("{name}\\{PLAYER_EXE} is not a regular file."),
            )),
            FileState::Inaccessible => problems.push((
                ProbeOutcome::Inaccessible,
                format!("{name}\\{PLAYER_EXE} could not be checked."),
            )),
        }
    }
    // Newest file first; folders whose date is unknown go last; ties break on name for stability.
    verified.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then_with(|| a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()))
    });

    // Decided from every problem, including the ones the findings cap hides below.
    let unresolved = !complete
        || problems
            .iter()
            .any(|(o, _)| *o == ProbeOutcome::Inaccessible);

    let shown = problems.len().min(MAX_REPORTED_PROBLEMS);
    for (outcome, detail) in problems.iter().take(shown) {
        findings.push(finding(Src, *outcome, detail.clone()));
    }
    if problems.len() > shown {
        findings.push(summarise_hidden_problems(&problems[shown..]));
    }

    if verified.is_empty() {
        if candidates.is_empty() && complete {
            findings.push(finding(
                Src,
                ProbeOutcome::NotFound,
                "The Versions folder contains no Roblox version folders.",
            ));
        }
        return unresolved;
    }
    findings.push(finding(
        Src,
        ProbeOutcome::Found,
        format!("Found {} version folder(s) containing a non-empty {PLAYER_EXE}; the most recently updated is listed first. Contents were not checked.", verified.len()),
    ));
    for (name, exe, modified_ms) in verified {
        let path = exe.to_string_lossy().into_owned();
        if installs
            .iter()
            .any(|i| same_path(&i.executable_path, &path))
        {
            continue;
        }
        installs.push(RobloxInstall {
            source: Src,
            version: Some(name),
            executable_path: path,
            modified_at: modified_ms,
        });
    }
    unresolved
}

/// One neutral line for the version-folder problems beyond [`MAX_REPORTED_PROBLEMS`]. They can be
/// of different kinds, so the outcome is `Several` and the text counts each kind (zero counts are
/// left out). It never labels a mix as one kind.
fn summarise_hidden_problems(hidden: &[(ProbeOutcome, String)]) -> Finding {
    let count = |kind: ProbeOutcome| hidden.iter().filter(|(o, _)| *o == kind).count();
    let parts: Vec<String> = [
        (
            count(ProbeOutcome::MissingExecutable),
            format!("without {PLAYER_EXE}"),
        ),
        (
            count(ProbeOutcome::NotAFile),
            format!("with an empty or non-file {PLAYER_EXE}"),
        ),
        (
            count(ProbeOutcome::Inaccessible),
            "that could not be checked".to_owned(),
        ),
    ]
    .into_iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, what)| format!("{n} {what}"))
    .collect();
    finding(
        DetectionSource::VersionFolder,
        ProbeOutcome::Several,
        format!(
            "{} more version folder(s) had problems: {}.",
            hidden.len(),
            parts.join(", ")
        ),
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::HashMap;

    const HASH_A: &str = "version-0123456789abcdef";
    const HASH_B: &str = "version-fedcba9876543210";
    const HASH_C: &str = "version-00000000deadbeef";
    const LAUNCHER: &str = r"C:\Users\me\AppData\Local\Roblox\Versions\version-0123456789abcdef\RobloxPlayerLauncher.exe";

    /// A pretend computer.
    pub(crate) struct Fake {
        pub(crate) supported: bool,
        pub(crate) versions: Option<PathBuf>,
        pub(crate) dirs: HashMap<PathBuf, DirListing>,
        pub(crate) files: HashMap<PathBuf, FileState>,
        pub(crate) handler: RegistryValue,
    }

    impl Fake {
        pub(crate) fn windows() -> Self {
            Fake {
                supported: true,
                versions: Some(PathBuf::from("root").join("Versions")),
                dirs: HashMap::new(),
                files: HashMap::new(),
                handler: RegistryValue::NotSet,
            }
        }
        fn versions(&self) -> PathBuf {
            self.versions.clone().unwrap()
        }
        /// A Versions folder holding the given sub-folders.
        pub(crate) fn with_folders(mut self, names: &[&str]) -> Self {
            let listing = DirListing::Entries {
                names: names.iter().map(|s| s.to_string()).collect(),
                complete: true,
            };
            self.dirs.insert(self.versions(), listing);
            self
        }
        pub(crate) fn exe(&self, folder: &str) -> PathBuf {
            self.versions().join(folder).join(PLAYER_EXE)
        }
        pub(crate) fn with_exe(mut self, folder: &str, state: FileState) -> Self {
            let p = self.exe(folder);
            self.files.insert(p, state);
            self
        }
        pub(crate) fn with_handler(mut self, value: RegistryValue) -> Self {
            self.handler = value;
            self
        }
        pub(crate) fn with_file(mut self, path: &str, state: FileState) -> Self {
            self.files.insert(PathBuf::from(path), state);
            self
        }
    }

    impl Environment for Fake {
        fn platform_supported(&self) -> bool {
            self.supported
        }
        fn versions_dir(&self) -> Option<PathBuf> {
            self.versions.clone()
        }
        fn list_dirs(&self, dir: &Path) -> DirListing {
            self.dirs.get(dir).cloned().unwrap_or(DirListing::Missing)
        }
        fn file_state(&self, path: &Path) -> FileState {
            self.files.get(path).copied().unwrap_or(FileState::Missing)
        }
        fn protocol_handler_command(&self) -> RegistryValue {
            self.handler.clone()
        }
    }

    pub(crate) fn file(ms: Option<i64>) -> FileState {
        FileState::File { modified_ms: ms }
    }

    fn outcomes(d: &RobloxDetection, source: DetectionSource) -> Vec<ProbeOutcome> {
        d.findings
            .iter()
            .filter(|f| f.source == source)
            .map(|f| f.outcome)
            .collect()
    }

    // ---- platform ----

    #[test]
    fn other_operating_systems_are_reported_unsupported() {
        let mut env = Fake::windows();
        env.supported = false;
        let d = detect(&env);
        assert_eq!(d.status, DetectionStatus::Unsupported);
        assert!(d.installs.is_empty());
        assert_eq!(
            outcomes(&d, DetectionSource::Platform),
            [ProbeOutcome::Unsupported]
        );
    }

    #[test]
    fn microsoft_store_is_never_claimed_as_supported() {
        let env = Fake::windows()
            .with_folders(&[HASH_A])
            .with_exe(HASH_A, file(Some(1)));
        let d = detect(&env);
        assert_eq!(d.status, DetectionStatus::Detected);
        assert_eq!(
            outcomes(&d, DetectionSource::MicrosoftStore),
            [ProbeOutcome::Unsupported]
        );
        assert!(
            d.installs
                .iter()
                .all(|i| i.source != DetectionSource::MicrosoftStore)
        );
    }

    // ---- version folders ----

    #[test]
    fn nothing_installed_is_a_clean_no() {
        let d = detect(&Fake::windows()); // no Versions folder, no handler
        assert_eq!(d.status, DetectionStatus::NotDetected);
        assert!(d.installs.is_empty());
        assert_eq!(
            outcomes(&d, DetectionSource::VersionFolder),
            [ProbeOutcome::NotFound]
        );
        assert_eq!(
            outcomes(&d, DetectionSource::ProtocolHandler),
            [ProbeOutcome::NotFound]
        );
    }

    #[test]
    fn empty_versions_folder_is_a_clean_no() {
        let d = detect(&Fake::windows().with_folders(&[]));
        assert_eq!(d.status, DetectionStatus::NotDetected);
    }

    #[test]
    fn a_version_folder_is_detected_only_when_its_program_exists() {
        let env = Fake::windows()
            .with_folders(&[HASH_A])
            .with_exe(HASH_A, file(Some(5)));
        let d = detect(&env);
        assert_eq!(d.status, DetectionStatus::Detected);
        assert_eq!(d.installs.len(), 1);
        assert_eq!(d.installs[0].source, DetectionSource::VersionFolder);
        assert_eq!(d.installs[0].version.as_deref(), Some(HASH_A));
        assert_eq!(d.installs[0].modified_at, Some(5));
        assert!(d.installs[0].executable_path.ends_with(PLAYER_EXE));
    }

    #[test]
    fn highest_sorting_name_is_not_chosen_without_a_program() {
        // "version-fedc…" sorts after "version-0123…" but has no program file.
        let env = Fake::windows()
            .with_folders(&[HASH_A, HASH_B])
            .with_exe(HASH_A, file(Some(1)));
        let d = detect(&env);
        assert_eq!(d.installs.len(), 1);
        assert_eq!(d.installs[0].version.as_deref(), Some(HASH_A));
        assert!(
            outcomes(&d, DetectionSource::VersionFolder).contains(&ProbeOutcome::MissingExecutable)
        );
    }

    #[test]
    fn only_folders_with_no_program_means_not_detected_with_the_reason() {
        let d = detect(&Fake::windows().with_folders(&[HASH_A, HASH_B]));
        assert_eq!(d.status, DetectionStatus::NotDetected);
        let o = outcomes(&d, DetectionSource::VersionFolder);
        assert_eq!(
            o.iter()
                .filter(|x| **x == ProbeOutcome::MissingExecutable)
                .count(),
            2
        );
    }

    #[test]
    fn newest_file_wins_regardless_of_name_order() {
        let env = Fake::windows()
            .with_folders(&[HASH_A, HASH_B, HASH_C])
            .with_exe(HASH_A, file(Some(100))) // oldest, smallest-ish name
            .with_exe(HASH_B, file(Some(300))) // newest, highest name
            .with_exe(HASH_C, file(Some(200)));
        let versions: Vec<_> = detect(&env)
            .installs
            .iter()
            .map(|i| i.version.clone().unwrap())
            .collect();
        assert_eq!(versions, [HASH_B, HASH_C, HASH_A]);

        // Flip the dates: the answer follows the dates, not the names.
        let env = Fake::windows()
            .with_folders(&[HASH_A, HASH_B, HASH_C])
            .with_exe(HASH_A, file(Some(900)))
            .with_exe(HASH_B, file(Some(300)))
            .with_exe(HASH_C, file(Some(200)));
        assert_eq!(detect(&env).installs[0].version.as_deref(), Some(HASH_A));
    }

    #[test]
    fn unknown_dates_sort_last_and_ties_are_stable() {
        let env = Fake::windows()
            .with_folders(&[HASH_B, HASH_A, HASH_C])
            .with_exe(HASH_A, file(None))
            .with_exe(HASH_B, file(Some(7)))
            .with_exe(HASH_C, file(Some(7)));
        let versions: Vec<_> = detect(&env)
            .installs
            .iter()
            .map(|i| i.version.clone().unwrap())
            .collect();
        // B and C tie on date, so name order decides (c < f); the undated A is last.
        assert_eq!(versions, [HASH_C, HASH_B, HASH_A]);
    }

    #[test]
    fn invalid_version_folder_names_are_ignored_and_reported() {
        let names = [
            "version-",
            "version-xyz12345678",
            "versions",
            "Version-short",
            "version-0123456789abcdefg",
            "version-0123456789abcdef0123456789abcdef0",
            "ünïcode-folder",
            HASH_A,
        ];
        let env = Fake::windows()
            .with_folders(&names)
            .with_exe(HASH_A, file(Some(1)));
        let d = detect(&env);
        assert_eq!(d.installs.len(), 1);
        let ignored = d
            .findings
            .iter()
            .find(|f| f.outcome == ProbeOutcome::Malformed)
            .expect("ignored names are reported");
        assert!(
            ignored.detail.starts_with("7 folder(s)"),
            "{}",
            ignored.detail
        );
    }

    #[test]
    fn version_names_follow_the_rule() {
        assert!(is_version_name("version-0123456789abcdef"));
        assert!(is_version_name("VERSION-0123456789ABCDEF"));
        assert!(is_version_name("version-12345678"));
        assert!(!is_version_name("version-1234567")); // too short
        assert!(!is_version_name(
            "version-0123456789abcdef0123456789abcdef0"
        )); // too long
        assert!(!is_version_name("version-0123456789abcdeg"));
        assert!(!is_version_name("../version-0123456789abcdef"));
        assert!(!is_version_name("version-0123456789abcd/\\"));
        assert!(!is_version_name(""));
        assert!(!is_version_name("ééééééééééééééééé")); // multi-byte text never panics
    }

    #[test]
    fn empty_or_non_file_programs_are_not_installs() {
        for state in [FileState::Empty, FileState::NotAFile] {
            let d = detect(
                &Fake::windows()
                    .with_folders(&[HASH_A])
                    .with_exe(HASH_A, state),
            );
            assert_eq!(d.status, DetectionStatus::NotDetected, "{state:?}");
            assert!(outcomes(&d, DetectionSource::VersionFolder).contains(&ProbeOutcome::NotAFile));
        }
    }

    #[test]
    fn unreadable_folder_is_inconclusive_not_a_no() {
        let mut env = Fake::windows();
        env.dirs.insert(env.versions(), DirListing::Inaccessible);
        let d = detect(&env);
        assert_eq!(d.status, DetectionStatus::Inconclusive);
        assert_eq!(
            outcomes(&d, DetectionSource::VersionFolder),
            [ProbeOutcome::Inaccessible]
        );
    }

    #[test]
    fn unknown_app_data_location_is_inconclusive() {
        let mut env = Fake::windows();
        env.versions = None;
        assert_eq!(detect(&env).status, DetectionStatus::Inconclusive);
    }

    #[test]
    fn unreadable_program_is_inconclusive_not_missing() {
        let d = detect(
            &Fake::windows()
                .with_folders(&[HASH_A])
                .with_exe(HASH_A, FileState::Inaccessible),
        );
        assert_eq!(d.status, DetectionStatus::Inconclusive);
    }

    #[test]
    fn incomplete_listing_is_inconclusive_unless_something_was_verified() {
        let mut env = Fake::windows();
        env.dirs.insert(
            env.versions(),
            DirListing::Entries {
                names: vec![],
                complete: false,
            },
        );
        assert_eq!(detect(&env).status, DetectionStatus::Inconclusive);

        let mut env = Fake::windows().with_exe(HASH_A, file(Some(1)));
        env.dirs.insert(
            env.versions(),
            DirListing::Entries {
                names: vec![HASH_A.into()],
                complete: false,
            },
        );
        let d = detect(&env);
        assert_eq!(d.status, DetectionStatus::Detected);
        assert!(outcomes(&d, DetectionSource::VersionFolder).contains(&ProbeOutcome::Incomplete));
    }

    fn version_names(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("version-{i:016x}")).collect()
    }

    fn summary(d: &RobloxDetection) -> &Finding {
        d.findings
            .iter()
            .find(|f| f.detail.contains("more version folder"))
            .expect("a summary line")
    }

    #[test]
    fn hidden_unreadable_problem_is_inconclusive_with_a_neutral_summary() {
        let names = version_names(15);
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut env = Fake::windows().with_folders(&refs);
        let last = env.exe(&names[14]); // beyond the cap, so not shown on its own
        env.files.insert(last, FileState::Inaccessible);
        let d = detect(&env);
        assert_eq!(
            d.findings
                .iter()
                .filter(|f| f.source == DetectionSource::VersionFolder)
                .count(),
            MAX_REPORTED_PROBLEMS + 1
        );
        let s = summary(&d);
        assert_eq!(s.outcome, ProbeOutcome::Several);
        assert_eq!(
            s.detail,
            "5 more version folder(s) had problems: 4 without RobloxPlayerBeta.exe, 1 that could not be checked."
        );
        assert_eq!(d.status, DetectionStatus::Inconclusive);
    }

    #[test]
    fn hidden_mixed_problems_are_never_labelled_missing() {
        let names = version_names(15);
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        // The first 10 are shown (all missing). Hidden: 2 empty, 1 not a file, 2 missing.
        let env = Fake::windows()
            .with_folders(&refs)
            .with_exe(&names[10], FileState::Empty)
            .with_exe(&names[11], FileState::Empty)
            .with_exe(&names[12], FileState::NotAFile);
        let d = detect(&env);
        let s = summary(&d);
        assert_eq!(s.outcome, ProbeOutcome::Several);
        assert_eq!(
            s.detail,
            "5 more version folder(s) had problems: 2 without RobloxPlayerBeta.exe, 3 with an empty or non-file RobloxPlayerBeta.exe."
        );
        // Nothing unreadable, so this is a clean "no".
        assert_eq!(d.status, DetectionStatus::NotDetected);
    }

    #[test]
    fn findings_stay_capped_for_500_problem_folders() {
        let names = version_names(500);
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let d = detect(&Fake::windows().with_folders(&refs));
        assert_eq!(
            d.findings
                .iter()
                .filter(|f| f.source == DetectionSource::VersionFolder)
                .count(),
            MAX_REPORTED_PROBLEMS + 1
        );
        assert_eq!(
            summary(&d).detail,
            "490 more version folder(s) had problems: 490 without RobloxPlayerBeta.exe."
        );
        assert_eq!(d.status, DetectionStatus::NotDetected);
    }

    #[test]
    fn ignored_folder_names_do_not_make_it_inconclusive() {
        let d = detect(&Fake::windows().with_folders(&["junk", "versions", "Temp"]));
        assert_eq!(d.status, DetectionStatus::NotDetected);
        assert!(outcomes(&d, DetectionSource::VersionFolder).contains(&ProbeOutcome::Malformed));
    }

    #[test]
    fn found_details_never_claim_the_program_is_genuine() {
        let env = Fake::windows()
            .with_folders(&[HASH_A])
            .with_exe(HASH_A, file(Some(1)))
            .with_handler(handler(LAUNCHER))
            .with_file(LAUNCHER, file(Some(2)));
        let d = detect(&env);
        assert_eq!(d.status, DetectionStatus::Detected);
        for f in &d.findings {
            let lower = f.detail.to_lowercase();
            for word in ["verified", "genuine", "valid"] {
                assert!(!lower.contains(word), "{word:?} in {:?}", f.detail);
            }
        }
        let found: Vec<_> = d
            .findings
            .iter()
            .filter(|f| f.outcome == ProbeOutcome::Found)
            .collect();
        assert_eq!(found.len(), 2);
        assert!(
            found
                .iter()
                .all(|f| f.detail.contains("Contents were not checked."))
        );
    }

    // ---- protocol handler ----

    fn handler(cmd: &str) -> RegistryValue {
        RegistryValue::Text(cmd.to_owned())
    }

    #[test]
    fn quoted_handler_with_arguments_is_detected() {
        let cmd = format!("\"{LAUNCHER}\" %1");
        let env = Fake::windows()
            .with_handler(handler(&cmd))
            .with_file(LAUNCHER, file(Some(9)));
        let d = detect(&env);
        assert_eq!(d.status, DetectionStatus::Detected);
        assert_eq!(d.installs[0].source, DetectionSource::ProtocolHandler);
        assert_eq!(d.installs[0].executable_path, LAUNCHER);
        assert_eq!(d.installs[0].version.as_deref(), Some(HASH_A));
        // Arguments never reach the result.
        assert!(d.findings.iter().all(|f| !f.detail.contains("%1")));
    }

    #[test]
    fn unquoted_handler_with_spaces_in_the_path_is_detected() {
        let prog = r"C:\Program Files (x86)\Roblox Inc\Versions\version-0123456789abcdef\RobloxPlayerLauncher.exe";
        let cmd = format!("{prog} --app -j \"%1\"");
        let env = Fake::windows()
            .with_handler(handler(&cmd))
            .with_file(prog, file(None));
        let d = detect(&env);
        assert_eq!(d.installs[0].executable_path, prog);
    }

    #[test]
    fn handler_file_name_check_is_case_insensitive() {
        let prog = r"c:\x\ROBLOXPLAYERLAUNCHER.EXE";
        let env = Fake::windows()
            .with_handler(handler(prog))
            .with_file(prog, file(None));
        assert_eq!(detect(&env).status, DetectionStatus::Detected);
    }

    #[test]
    fn handler_pointing_at_a_missing_program_is_reported_as_such() {
        let env = Fake::windows().with_handler(handler(&format!("\"{LAUNCHER}\" %1")));
        let d = detect(&env);
        assert_eq!(d.status, DetectionStatus::NotDetected);
        assert_eq!(
            outcomes(&d, DetectionSource::ProtocolHandler),
            [ProbeOutcome::MissingExecutable]
        );
    }

    #[test]
    fn handler_with_empty_or_unreadable_program() {
        let q = format!("\"{LAUNCHER}\"");
        let d = detect(
            &Fake::windows()
                .with_handler(handler(&q))
                .with_file(LAUNCHER, FileState::Empty),
        );
        assert_eq!(
            outcomes(&d, DetectionSource::ProtocolHandler),
            [ProbeOutcome::NotAFile]
        );
        let d = detect(
            &Fake::windows()
                .with_handler(handler(&q))
                .with_file(LAUNCHER, FileState::Inaccessible),
        );
        assert_eq!(d.status, DetectionStatus::Inconclusive);
    }

    #[test]
    fn handler_for_another_program_is_not_counted_as_roblox() {
        let prog = r"C:\Tools\Bloxstrap\Bloxstrap.exe";
        let env = Fake::windows()
            .with_handler(handler(&format!("\"{prog}\" \"%1\"")))
            .with_file(prog, file(Some(1)));
        let d = detect(&env);
        assert_eq!(d.status, DetectionStatus::NotDetected);
        assert_eq!(
            outcomes(&d, DetectionSource::ProtocolHandler),
            [ProbeOutcome::Unrecognised]
        );
    }

    #[test]
    fn registry_not_set_vs_inaccessible_vs_malformed() {
        // Nothing registered (and no Versions folder): a clean no.
        let d = detect(&Fake::windows().with_handler(RegistryValue::NotSet));
        assert_eq!(d.status, DetectionStatus::NotDetected);
        // Could not read it: unknown, not "no".
        let d = detect(&Fake::windows().with_handler(RegistryValue::Inaccessible));
        assert_eq!(d.status, DetectionStatus::Inconclusive);
        // Present but not understood: also unknown, not "no".
        let d = detect(&Fake::windows().with_handler(RegistryValue::Malformed));
        assert_eq!(d.status, DetectionStatus::Inconclusive);
        assert_eq!(
            outcomes(&d, DetectionSource::ProtocolHandler),
            [ProbeOutcome::Malformed]
        );
    }

    #[test]
    fn an_unclear_handler_never_hides_an_install_found_another_way() {
        for (value, expected) in [
            (RegistryValue::Malformed, ProbeOutcome::Malformed),
            (RegistryValue::Inaccessible, ProbeOutcome::Inaccessible),
            (handler("\"unterminated"), ProbeOutcome::Malformed),
        ] {
            let env = Fake::windows()
                .with_folders(&[HASH_A])
                .with_exe(HASH_A, file(Some(1)))
                .with_handler(value.clone());
            let d = detect(&env);
            assert_eq!(d.status, DetectionStatus::Detected, "{value:?}");
            assert_eq!(d.installs.len(), 1);
            // The handler problem is still reported.
            assert_eq!(
                outcomes(&d, DetectionSource::ProtocolHandler),
                [expected],
                "{value:?}"
            );
        }
    }

    #[test]
    fn unclear_version_folders_never_hide_a_handler_install() {
        let mut env = Fake::windows()
            .with_handler(handler(LAUNCHER))
            .with_file(LAUNCHER, file(Some(1)));
        env.dirs.insert(env.versions(), DirListing::Inaccessible);
        assert_eq!(detect(&env).status, DetectionStatus::Detected);

        let mut env = Fake::windows()
            .with_handler(handler(LAUNCHER))
            .with_file(LAUNCHER, file(Some(1)));
        env.versions = None;
        assert_eq!(detect(&env).status, DetectionStatus::Detected);
    }

    #[test]
    fn a_clean_no_from_one_source_does_not_cancel_an_unclear_other_source() {
        // Handler clean no; folders unreadable.
        let mut env = Fake::windows().with_handler(RegistryValue::NotSet);
        env.dirs.insert(env.versions(), DirListing::Inaccessible);
        assert_eq!(detect(&env).status, DetectionStatus::Inconclusive);

        // Handler malformed; no Versions folder (clean no).
        let env = Fake::windows().with_handler(RegistryValue::Malformed);
        assert_eq!(detect(&env).status, DetectionStatus::Inconclusive);

        // Handler unparsable; folder programs all missing (clean no).
        let env = Fake::windows()
            .with_folders(&[HASH_A])
            .with_handler(handler("\"C:\\a\\x.exe\"junk"));
        assert_eq!(detect(&env).status, DetectionStatus::Inconclusive);

        // Handler points at another program (clean no); a folder program is unreadable.
        let env = Fake::windows()
            .with_folders(&[HASH_A])
            .with_exe(HASH_A, FileState::Inaccessible)
            .with_handler(handler(r"C:\Tools\Other\Other.exe"));
        assert_eq!(detect(&env).status, DetectionStatus::Inconclusive);
    }

    #[test]
    fn malformed_registry_commands_are_rejected_never_run_or_followed() {
        let long = format!("\"C:\\{}.exe\"", "a".repeat(5000));
        let cases: Vec<(String, CommandError)> = vec![
            ("".into(), CommandError::Empty),
            ("   \t ".into(), CommandError::Empty),
            (
                "\"C:\\a\\RobloxPlayerLauncher.exe".into(),
                CommandError::UnterminatedQuote,
            ),
            (
                "\"C:\\a\\RobloxPlayerLauncher.exe\"x".into(),
                CommandError::UnexpectedText,
            ),
            ("\"\" %1".into(), CommandError::Empty),
            (
                "C:\\a\\RobloxPlayerLauncher.txt %1".into(),
                CommandError::NoExecutable,
            ),
            (
                "RobloxPlayerLauncher.exe %1".into(),
                CommandError::NotLocalPath,
            ),
            (
                "..\\RobloxPlayerLauncher.exe".into(),
                CommandError::NotLocalPath,
            ),
            (
                "\\\\server\\share\\RobloxPlayerLauncher.exe".into(),
                CommandError::NotLocalPath,
            ),
            (
                "\\\\?\\C:\\a\\RobloxPlayerLauncher.exe".into(),
                CommandError::NotLocalPath,
            ),
            (
                "%LOCALAPPDATA%\\Roblox\\RobloxPlayerLauncher.exe".into(),
                CommandError::NotLocalPath,
            ),
            (
                "C:\\a\\..\\b\\RobloxPlayerLauncher.exe".into(),
                CommandError::UnsafePath,
            ),
            (
                "C:\\a\\RobloxPlayerLauncher.exe:stream".into(),
                CommandError::NoExecutable,
            ),
            (
                "\"C:\\a\\b|c\\RobloxPlayerLauncher.exe\"".into(),
                CommandError::UnsafePath,
            ),
            (
                "\"C:\\a\\<b>\\RobloxPlayerLauncher.exe\"".into(),
                CommandError::UnsafePath,
            ),
            ("\"C:\\a\\.exe\"".into(), CommandError::NoExecutable),
            (
                "\"C:\\a\\x.exe\u{0}\"".into(),
                CommandError::InvalidCharacter,
            ),
            (
                "\"C:\\a\\x.exe\"\n&calc".into(),
                CommandError::InvalidCharacter,
            ),
            (long, CommandError::TooLong),
        ];
        for (input, expected) in cases {
            assert_eq!(
                parse_command(&input),
                Err(expected),
                "input: {:?}",
                input.chars().take(60).collect::<String>()
            );
        }
    }

    #[test]
    fn parse_command_extracts_only_the_program() {
        assert_eq!(
            parse_command(r#""C:\a b\x.exe" -flag "C:\other.exe""#).as_deref(),
            Ok(r"C:\a b\x.exe")
        );
        assert_eq!(
            parse_command(r"C:\a b\x.exe -flag").as_deref(),
            Ok(r"C:\a b\x.exe")
        );
        assert_eq!(parse_command(r"  C:\x.exe  ").as_deref(), Ok(r"C:\x.exe"));
        assert_eq!(
            parse_command("C:/forward/slashes/x.exe").as_deref(),
            Ok("C:/forward/slashes/x.exe")
        );
        // ".exe" inside a folder name does not end an unquoted program early.
        assert_eq!(
            parse_command(r"C:\my.exe.backup\x.exe /s").as_deref(),
            Ok(r"C:\my.exe.backup\x.exe")
        );
        assert_eq!(
            parse_command(r"C:\Käse\x.EXE /s").as_deref(),
            Ok(r"C:\Käse\x.EXE")
        );
    }

    #[test]
    fn unparsable_handler_text_is_inconclusive_not_an_install() {
        let d = detect(&Fake::windows().with_handler(handler("\"unterminated")));
        assert!(d.installs.is_empty());
        assert_eq!(d.status, DetectionStatus::Inconclusive);
        assert_eq!(
            outcomes(&d, DetectionSource::ProtocolHandler),
            [ProbeOutcome::Malformed]
        );
    }

    #[test]
    fn version_from_path_needs_a_version_parent() {
        assert_eq!(version_from_path(LAUNCHER).as_deref(), Some(HASH_A));
        assert_eq!(
            version_from_path(r"C:\Roblox\RobloxPlayerLauncher.exe"),
            None
        );
        assert_eq!(version_from_path("RobloxPlayerLauncher.exe"), None);
    }

    // ---- combining ----

    #[test]
    fn handler_install_is_listed_before_the_folder_install() {
        let env = Fake::windows()
            .with_folders(&[HASH_A])
            .with_exe(HASH_A, file(Some(1)))
            .with_handler(handler(LAUNCHER))
            .with_file(LAUNCHER, file(Some(2)));
        let d = detect(&env);
        assert_eq!(d.installs.len(), 2);
        assert_eq!(d.installs[0].source, DetectionSource::ProtocolHandler);
        assert_eq!(d.installs[1].source, DetectionSource::VersionFolder);
    }

    #[test]
    fn the_same_file_from_both_sources_is_listed_once() {
        let mut env = Fake::windows()
            .with_folders(&[HASH_A])
            .with_exe(HASH_A, file(Some(1)));
        env.versions = Some(PathBuf::from(r"C:\Roblox\Versions"));
        env.dirs.insert(
            env.versions(),
            DirListing::Entries {
                names: vec![HASH_A.into()],
                complete: true,
            },
        );
        let exe = env.exe(HASH_A);
        env.files.insert(exe.clone(), file(Some(1)));
        let env = env.with_handler(RegistryValue::Text(exe.to_string_lossy().into_owned()));
        let d = detect(&env);
        assert_eq!(d.installs.len(), 1, "{:?}", d.installs);
        assert_eq!(d.installs[0].source, DetectionSource::ProtocolHandler);
    }

    #[test]
    fn forward_slash_handler_path_is_deduplicated_against_the_folder_scan() {
        let mut env = Fake::windows()
            .with_folders(&[HASH_A])
            .with_exe(HASH_A, file(Some(1)));
        env.versions = Some(PathBuf::from(r"C:\Roblox\Versions"));
        env.dirs.insert(
            env.versions(),
            DirListing::Entries {
                names: vec![HASH_A.into()],
                complete: true,
            },
        );
        let exe = env.exe(HASH_A);
        env.files.insert(exe.clone(), file(Some(1)));
        let slashed = exe.to_string_lossy().replace('\\', "/");
        assert!(slashed.contains('/') && !slashed.contains('\\'));
        let env = env
            .with_handler(handler(&slashed))
            .with_file(&slashed, file(Some(1)));
        let d = detect(&env);
        assert_eq!(d.installs.len(), 1, "{:?}", d.installs);
        assert_eq!(d.installs[0].source, DetectionSource::ProtocolHandler);
    }

    #[test]
    fn same_path_ignores_ascii_case_and_slash_direction_only() {
        assert!(same_path(r"C:\A\b.exe", "c:/a/B.EXE"));
        assert!(!same_path(r"C:\A\b.exe", r"C:\A\c.exe"));
        assert!(!same_path(r"C:\A\b.exe", r"C:\A\b.exe\")); // length differs
        assert!(!same_path(r"C:\Käse\x.exe", r"C:\KÄSE\x.exe")); // no Unicode folding
    }

    #[test]
    fn result_serialises_with_camel_case_names_for_the_frontend() {
        let d = detect(&Fake::windows());
        let json = serde_json::to_value(&d).unwrap();
        assert_eq!(json["status"], "notDetected");
        assert_eq!(json["findings"][0]["source"], "protocolHandler");
        assert_eq!(json["findings"][0]["outcome"], "notFound");
        let inst = serde_json::to_value(RobloxInstall {
            source: DetectionSource::VersionFolder,
            version: None,
            executable_path: "x".into(),
            modified_at: Some(1),
        })
        .unwrap();
        assert_eq!(inst["source"], "versionFolder");
        assert_eq!(inst["executablePath"], "x");
        assert_eq!(inst["modifiedAt"], 1);
    }
}
