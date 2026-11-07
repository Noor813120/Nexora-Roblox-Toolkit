//! Which Roblox installation the user prefers (Phase 4, Milestone 2): the pure rules.
//!
//! Three things are kept apart on purpose:
//! - *Detection* ([`super::roblox`]) says what exists on this computer. Nothing here changes it.
//! - The *stored preference* says what the user picked: a **source kind**, never a path, because
//!   Roblox replaces its version folders on every update.
//! - *Resolution* ([`resolve`]) compares the two, fresh each time. A stored preference is never
//!   proof that an installation exists, and a stale one never makes detection look negative.
//!
//! Everything here is a pure function of its inputs: no disk, no database, no registry. The
//! database side lives in `storage::settings`, and the command that ties them together comes later.
//!
//! Rules (agreed before implementation):
//! - `protocolHandler` resolves to the install the Windows link handler points to.
//! - `versionFolder` resolves to the **newest** install whose program is `RobloxPlayerBeta.exe`
//!   inside a version folder. That includes the folder the link handler points to, because
//!   detection lists it once, under the handler.
//! - Several candidates is not an error. The pick is deterministic (see [`order`]) and the count is
//!   reported, so the screen can say "1 of N".
//! - A preference that cannot be resolved is reported as such. It never falls back to another
//!   installation; only "automatic" (nothing stored) uses detection's primary entry.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use ts_rs::TS;

use super::roblox::{
    DetectionSource, DetectionStatus, PLAYER_EXE, ProbeOutcome, RobloxDetection, RobloxInstall,
};

/// The settings key that holds the stored preference.
pub const SETTING_KEY: &str = "launcher.preferred_install";
/// The activity-log event written when the preference changes.
pub const EVENT_TYPE: &str = "launcher.preferred_install_changed";
/// Stored values longer than this are not even parsed. A real one is under 50 characters.
const MAX_STORED_LEN: usize = 256;

const AUTOMATIC: &str = "automatic";
const UNRECOGNISED: &str = "unrecognised";

// ---------------------------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------------------------

/// The kinds of source a user can prefer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PreferredSource {
    /// The install the Windows `roblox-player` link handler points to.
    ProtocolHandler,
    /// The newest install inside a Roblox version folder.
    VersionFolder,
}

impl PreferredSource {
    /// The name stored on disk and sent to the screen (the same as the serde name).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProtocolHandler => "protocolHandler",
            Self::VersionFolder => "versionFolder",
        }
    }

    /// Plain-language name for messages.
    fn label(self) -> &'static str {
        match self {
            Self::ProtocolHandler => "Windows link handler",
            Self::VersionFolder => "Roblox version folders",
        }
    }

    fn detection_source(self) -> DetectionSource {
        match self {
            Self::ProtocolHandler => DetectionSource::ProtocolHandler,
            Self::VersionFolder => DetectionSource::VersionFolder,
        }
    }
}

/// What the stored value means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredPreference {
    /// Nothing stored: "automatic".
    Absent,
    Source(PreferredSource),
    /// Understood, but names a source that cannot be preferred (`microsoftStore`, `platform`).
    UnsupportedSource,
    /// Written by a newer version of the app (`v` above 1). Kept untouched.
    NewerFormat,
    /// Not something this app wrote: bad JSON, wrong shape, unknown source, too long.
    Invalid,
}

/// How the stored preference stands against what was just detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PreferenceState {
    /// Nothing stored; detection's primary install is used.
    Automatic,
    /// The preferred source resolves to an install that passed the basic checks.
    Available,
    /// The preferred source was checked and has nothing: gone, moved, or never there.
    Unavailable,
    /// Nothing for the preferred source, but something could not be checked, so it may exist.
    Inconclusive,
    /// Stored value is understood but cannot be used here; see [`PreferenceReason`].
    Unsupported,
    /// Stored value is not something this app wrote.
    Invalid,
}

/// Why a stored preference is [`PreferenceState::Unsupported`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PreferenceReason {
    NewerFormat,
    UnsupportedSource,
    UnsupportedPlatform,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PreferenceResolution {
    pub state: PreferenceState,
    /// The stored source, when the stored value could be read.
    pub preferred: Option<PreferredSource>,
    /// `Available`: the resolved install. `Automatic`: detection's primary install. Otherwise
    /// `None`. Runtime information only; it is never stored.
    pub chosen: Option<RobloxInstall>,
    /// How many installs the stored source offers (after removing duplicates). 0 when there is no
    /// stored source or it offers none.
    pub candidate_count: u32,
    /// Set only with [`PreferenceState::Unsupported`].
    pub reason: Option<PreferenceReason>,
}

// ---------------------------------------------------------------------------------------------
// The stored value
// ---------------------------------------------------------------------------------------------

/// Reads the stored value. Never fails and never echoes the text: anything that is not a value
/// this app wrote becomes one of the non-`Source` variants, and the caller leaves it untouched.
///
/// The stored form is `{"v":1,"source":"protocolHandler"}`. Extra fields are ignored.
pub fn parse_stored(raw: Option<&str>) -> StoredPreference {
    use StoredPreference::*;
    let Some(raw) = raw else { return Absent };
    if raw.len() > MAX_STORED_LEN || raw.contains('\0') {
        return Invalid;
    }
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(raw) else {
        return Invalid;
    };
    match map.get("v").and_then(serde_json::Value::as_u64) {
        Some(1) => match map.get("source").and_then(serde_json::Value::as_str) {
            Some("protocolHandler") => Source(PreferredSource::ProtocolHandler),
            Some("versionFolder") => Source(PreferredSource::VersionFolder),
            Some("microsoftStore" | "platform") => UnsupportedSource,
            _ => Invalid,
        },
        Some(newer) if newer > 1 => NewerFormat,
        _ => Invalid,
    }
}

/// The one way a preference is written: always this exact text, so equal choices are equal text.
pub fn canonical(source: PreferredSource) -> String {
    format!(r#"{{"v":1,"source":"{}"}}"#, source.as_str())
}

/// A change to make, described without any storage types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferenceChange {
    /// The text to store, or `None` to remove the setting ("automatic").
    pub new_raw: Option<String>,
    /// `automatic`, a source name, or `unrecognised`. Never any stored text.
    pub from: &'static str,
    pub to: &'static str,
    /// The activity-log message.
    pub message: String,
}

impl PreferenceChange {
    /// The activity entry's metadata: only the two labels.
    pub fn metadata_json(&self) -> String {
        format!(r#"{{"from":"{}","to":"{}"}}"#, self.from, self.to)
    }
}

/// Decides whether choosing `target` changes anything, given what is stored.
///
/// "Unchanged" means the same *meaning*, not the same text: nothing stored and "automatic", or the
/// same valid source. Anything else is a change, including replacing a value this app cannot read
/// (which is reported as coming from `unrecognised`).
pub fn plan_change(
    previous_raw: Option<&str>,
    target: Option<PreferredSource>,
) -> Option<PreferenceChange> {
    let previous = parse_stored(previous_raw);
    match (previous, target) {
        (StoredPreference::Absent, None) => return None,
        (StoredPreference::Source(a), Some(b)) if a == b => return None,
        _ => {}
    }
    let from = match previous {
        StoredPreference::Absent => AUTOMATIC,
        StoredPreference::Source(s) => s.as_str(),
        StoredPreference::UnsupportedSource
        | StoredPreference::NewerFormat
        | StoredPreference::Invalid => UNRECOGNISED,
    };
    let (to, message) = match target {
        None => (
            AUTOMATIC,
            "Preferred Roblox installation reset to automatic.".to_owned(),
        ),
        Some(s) => (
            s.as_str(),
            format!("Preferred Roblox installation set to {}.", s.label()),
        ),
    };
    Some(PreferenceChange {
        new_raw: target.map(canonical),
        from,
        to,
        message,
    })
}

// ---------------------------------------------------------------------------------------------
// Candidates
// ---------------------------------------------------------------------------------------------

/// Text-only path key: ASCII case and `/` versus `\` are ignored, as detection does when it merges
/// the same program found twice. Limit: ASCII only, so a path differing only in non-ASCII case
/// counts twice; the worst outcome is a candidate count one too high.
fn path_key(path: &str) -> String {
    path.chars()
        .map(|c| {
            if c == '/' {
                '\\'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

fn source_rank(source: DetectionSource) -> u8 {
    match source {
        DetectionSource::ProtocolHandler => 0,
        DetectionSource::VersionFolder => 1,
        DetectionSource::MicrosoftStore => 2,
        DetectionSource::Platform => 3,
    }
}

/// A total order over installs, so the pick never depends on the order detection listed them in.
/// 1. newest program file first (unknown dates last);
/// 2. folder name, ASCII-lowercased;
/// 3. folder name exactly as written;
/// 4. program path exactly as written;
/// 5. source (only matters if the same program were listed under two sources).
///
/// Every field of an install is compared, so two installs compare equal only if identical.
fn order(a: &RobloxInstall, b: &RobloxInstall) -> Ordering {
    let lower = |i: &RobloxInstall| i.version.as_ref().map(|v| v.to_ascii_lowercase());
    b.modified_at
        .cmp(&a.modified_at)
        .then_with(|| lower(a).cmp(&lower(b)))
        .then_with(|| a.version.cmp(&b.version))
        .then_with(|| a.executable_path.cmp(&b.executable_path))
        .then_with(|| source_rank(a.source).cmp(&source_rank(b.source)))
}

/// Is this the game client inside a Roblox version folder, whichever source found it?
fn is_version_folder_client(install: &RobloxInstall) -> bool {
    install.version.is_some()
        && file_name(&install.executable_path).eq_ignore_ascii_case(PLAYER_EXE)
}

/// The installs the source offers, best first, with duplicates (the same program listed twice)
/// removed *before* anything is counted. Of duplicates, the first in [`order`] is kept.
fn candidates(detection: &RobloxDetection, source: PreferredSource) -> Vec<RobloxInstall> {
    let mut found: Vec<&RobloxInstall> = detection
        .installs
        .iter()
        .filter(|i| match source {
            PreferredSource::ProtocolHandler => i.source == DetectionSource::ProtocolHandler,
            PreferredSource::VersionFolder => is_version_folder_client(i),
        })
        .collect();
    found.sort_by(|a, b| order(a, b));
    let mut seen = HashSet::new();
    found.retain(|i| seen.insert(path_key(&i.executable_path)));
    found.into_iter().cloned().collect()
}

// ---------------------------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------------------------

/// What one finding says about a source that offered no install.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Evidence {
    /// A clean "no": the place was checked and is not there.
    Negative,
    /// Something could not be checked or understood, so Roblox may still be there.
    Unresolved,
    /// Says nothing about whether this source has an install.
    Neutral,
}

/// Mirrors how detection decides a source is "unresolved" (see `probe_protocol_handler` and
/// `probe_version_folders`), one arm per [`ProbeOutcome`] with no catch-all, so adding an outcome
/// forces a decision here.
fn evidence(source: PreferredSource, outcome: ProbeOutcome) -> Evidence {
    use Evidence::*;
    use PreferredSource as S;
    use ProbeOutcome as O;
    match outcome {
        O::NotFound | O::MissingExecutable | O::NotAFile => Negative,
        // The link handler opening some other program is a clean "no" for that source.
        O::Unrecognised => match source {
            S::ProtocolHandler => Negative,
            S::VersionFolder => Neutral,
        },
        // A link-handler value that made no sense could hide a real install. For the Versions
        // folder this outcome only means some folder names were ignored, which detection does
        // not count as unresolved either.
        O::Malformed => match source {
            S::ProtocolHandler => Unresolved,
            S::VersionFolder => Neutral,
        },
        // `Several` summarises hidden problems that may include unreadable ones; being cautious
        // can only turn "unavailable" into "inconclusive", never the other way round.
        // `Found` with no install to show is contradictory, so no clean "no" can be given.
        O::Inaccessible | O::Incomplete | O::Several | O::Found => Unresolved,
        O::Unsupported => Neutral,
    }
}

/// The state for a stored source that offered nothing. Without at least one clean "no" there is no
/// basis for saying "unavailable", so that is `Inconclusive` too.
fn state_without_candidates(
    detection: &RobloxDetection,
    source: PreferredSource,
) -> PreferenceState {
    let mut saw_negative = false;
    for finding in detection
        .findings
        .iter()
        .filter(|f| f.source == source.detection_source())
    {
        match evidence(source, finding.outcome) {
            Evidence::Unresolved => return PreferenceState::Inconclusive,
            Evidence::Negative => saw_negative = true,
            Evidence::Neutral => {}
        }
    }
    if saw_negative {
        PreferenceState::Unavailable
    } else {
        PreferenceState::Inconclusive
    }
}

/// Compares the stored preference with a fresh detection. Pure: it never changes `detection`, and
/// a problem with the preference never changes what detection reported.
///
/// Detection is a point-in-time observation: the answer can be out of date a moment later, so
/// anything that acts on it (launching, later) must check again immediately before acting.
pub fn resolve(detection: &RobloxDetection, stored: StoredPreference) -> PreferenceResolution {
    let none = |state, reason| PreferenceResolution {
        state,
        preferred: None,
        chosen: None,
        candidate_count: 0,
        reason,
    };
    match stored {
        StoredPreference::Absent => PreferenceResolution {
            chosen: detection.installs.first().cloned(),
            ..none(PreferenceState::Automatic, None)
        },
        StoredPreference::Invalid => none(PreferenceState::Invalid, None),
        StoredPreference::NewerFormat => none(
            PreferenceState::Unsupported,
            Some(PreferenceReason::NewerFormat),
        ),
        StoredPreference::UnsupportedSource => none(
            PreferenceState::Unsupported,
            Some(PreferenceReason::UnsupportedSource),
        ),
        StoredPreference::Source(source) => {
            if detection.status == DetectionStatus::Unsupported {
                return PreferenceResolution {
                    preferred: Some(source),
                    ..none(
                        PreferenceState::Unsupported,
                        Some(PreferenceReason::UnsupportedPlatform),
                    )
                };
            }
            let found = candidates(detection, source);
            match found.first() {
                Some(best) => PreferenceResolution {
                    state: PreferenceState::Available,
                    preferred: Some(source),
                    chosen: Some(best.clone()),
                    candidate_count: u32::try_from(found.len()).unwrap_or(u32::MAX),
                    reason: None,
                },
                None => PreferenceResolution {
                    preferred: Some(source),
                    ..none(state_without_candidates(detection, source), None)
                },
            }
        }
    }
}

/// The sources that would currently be accepted as a choice, in a fixed order, from one
/// detection: exactly those for which [`resolve`] gives `Available`. Empty on an unsupported
/// system. Pure: it works from a detection that was already made and never detects again.
pub fn available_sources(detection: &RobloxDetection) -> Vec<PreferredSource> {
    [
        PreferredSource::ProtocolHandler,
        PreferredSource::VersionFolder,
    ]
    .into_iter()
    .filter(|source| {
        resolve(detection, StoredPreference::Source(*source)).state == PreferenceState::Available
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::roblox::tests::{Fake, file};
    use crate::launcher::roblox::{DirListing, FileState, Finding, RegistryValue, detect};
    use std::path::PathBuf;

    const PH: PreferredSource = PreferredSource::ProtocolHandler;
    const VF: PreferredSource = PreferredSource::VersionFolder;
    const A: &str = "version-aaaaaaaaaaaaaaaa";
    const B: &str = "version-bbbbbbbbbbbbbbbb";
    const C: &str = "version-cccccccccccccccc";
    const ROOT: &str = r"C:\Roblox\Versions";

    // ---- builders ----

    fn inst(
        source: DetectionSource,
        version: Option<&str>,
        path: &str,
        ms: Option<i64>,
    ) -> RobloxInstall {
        RobloxInstall {
            source,
            version: version.map(str::to_owned),
            executable_path: path.to_owned(),
            modified_at: ms,
        }
    }

    /// A version-folder install: `<dir>\<name>\RobloxPlayerBeta.exe`.
    fn vf(name: &str, dir: &str, ms: Option<i64>) -> RobloxInstall {
        inst(
            DetectionSource::VersionFolder,
            Some(name),
            &format!(r"{dir}\{name}\{PLAYER_EXE}"),
            ms,
        )
    }

    fn finding(source: DetectionSource, outcome: ProbeOutcome) -> Finding {
        Finding {
            source,
            outcome,
            detail: "x".into(),
        }
    }

    fn detection(installs: Vec<RobloxInstall>, findings: Vec<Finding>) -> RobloxDetection {
        RobloxDetection {
            status: if installs.is_empty() {
                DetectionStatus::NotDetected
            } else {
                DetectionStatus::Detected
            },
            installs,
            findings,
        }
    }

    /// A Windows machine whose Versions folder is `C:\Roblox\Versions`.
    fn machine() -> Fake {
        let mut env = Fake::windows();
        env.versions = Some(PathBuf::from(ROOT));
        env
    }

    fn with_handler_text(env: Fake, text: &str) -> Fake {
        env.with_handler(RegistryValue::Text(text.to_owned()))
    }

    /// The link handler points at `folder`'s game client (which must have been given a file).
    fn handler_to(env: Fake, folder: &str) -> Fake {
        let text = env.exe(folder).to_string_lossy().into_owned();
        with_handler_text(env, &text)
    }

    fn permutations<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
        if items.len() <= 1 {
            return vec![items.to_vec()];
        }
        let mut out = Vec::new();
        for i in 0..items.len() {
            let mut rest = items.to_vec();
            let head = rest.remove(i);
            for mut p in permutations(&rest) {
                p.insert(0, head.clone());
                out.push(p);
            }
        }
        out
    }

    fn path(r: &PreferenceResolution) -> Option<&str> {
        r.chosen.as_ref().map(|i| i.executable_path.as_str())
    }

    // ---- the stored value ----

    #[test]
    fn values_this_app_wrote_are_read() {
        assert_eq!(parse_stored(None), StoredPreference::Absent);
        for s in [PH, VF] {
            assert_eq!(
                parse_stored(Some(&canonical(s))),
                StoredPreference::Source(s)
            );
        }
        // Whitespace and key order do not matter; extra fields are ignored.
        let spaced = r#"{ "source" : "versionFolder" ,  "v" : 1 , "extra": [1, {"a": null}] }"#;
        assert_eq!(parse_stored(Some(spaced)), StoredPreference::Source(VF));
    }

    #[test]
    fn anything_else_is_invalid_and_never_panics() {
        let long = format!(
            r#"{{"v":1,"source":"versionFolder","pad":"{}"}}"#,
            "x".repeat(300)
        );
        let nul = "{\"v\":1,\"source\":\"versionFolder\"}\0";
        for raw in [
            "",
            "   ",
            "not json",
            "[1]",
            "null",
            "7",
            r#""protocolHandler""#,
            "{}",
            r#"{"v":1}"#,
            r#"{"source":"versionFolder"}"#,
            r#"{"v":1,"source":"nope"}"#,
            r#"{"v":1,"source":"VersionFolder"}"#, // names are case-sensitive
            r#"{"v":1,"source":5}"#,
            r#"{"v":1,"source":null}"#,
            r#"{"v":0,"source":"versionFolder"}"#,
            r#"{"v":-1,"source":"versionFolder"}"#,
            r#"{"v":1.0,"source":"versionFolder"}"#,
            r#"{"v":"1","source":"versionFolder"}"#,
            &long,
            nul,
        ] {
            assert_eq!(
                parse_stored(Some(raw)),
                StoredPreference::Invalid,
                "{raw:?}"
            );
        }
    }

    #[test]
    fn a_newer_format_is_recognised_without_reading_the_rest() {
        for raw in [
            r#"{"v":2,"source":"versionFolder"}"#,
            r#"{"v":2}"#,
            r#"{"v":2,"source":5}"#,
            r#"{"v":99,"garbage":[1,{"a":null}]}"#,
            r#"{"v":18446744073709551615}"#,
        ] {
            assert_eq!(
                parse_stored(Some(raw)),
                StoredPreference::NewerFormat,
                "{raw}"
            );
        }
    }

    #[test]
    fn sources_that_cannot_be_preferred_are_recognised() {
        for s in ["microsoftStore", "platform"] {
            let raw = format!(r#"{{"v":1,"source":"{s}"}}"#);
            assert_eq!(
                parse_stored(Some(&raw)),
                StoredPreference::UnsupportedSource
            );
        }
    }

    #[test]
    fn names_agree_between_disk_and_screen() {
        for s in [PH, VF] {
            assert_eq!(serde_json::to_value(s).unwrap(), s.as_str());
            assert_eq!(
                canonical(s),
                format!(r#"{{"v":1,"source":"{}"}}"#, s.as_str())
            );
        }
        // The frontend can only ever send the two real values.
        assert!(serde_json::from_str::<PreferredSource>(r#""microsoftStore""#).is_err());
        assert!(serde_json::from_str::<PreferredSource>(r#""nope""#).is_err());
        assert_eq!(
            serde_json::from_str::<PreferredSource>(r#""versionFolder""#).unwrap(),
            VF
        );
    }

    // ---- deciding whether a choice is a change ----

    #[test]
    fn choosing_what_is_already_chosen_is_not_a_change() {
        assert_eq!(plan_change(None, None), None, "nothing stored, automatic");
        for s in [PH, VF] {
            assert_eq!(plan_change(Some(&canonical(s)), Some(s)), None);
        }
        // Same meaning, different text.
        let spaced = r#"{ "v" : 1 , "source" : "protocolHandler" }"#;
        assert_eq!(plan_change(Some(spaced), Some(PH)), None);
    }

    #[test]
    fn real_changes_are_described() {
        let c = plan_change(None, Some(PH)).unwrap();
        assert_eq!(c.new_raw.as_deref(), Some(canonical(PH).as_str()));
        assert_eq!((c.from, c.to), ("automatic", "protocolHandler"));
        assert_eq!(
            c.message,
            "Preferred Roblox installation set to Windows link handler."
        );

        let c = plan_change(Some(&canonical(PH)), Some(VF)).unwrap();
        assert_eq!((c.from, c.to), ("protocolHandler", "versionFolder"));
        assert_eq!(
            c.message,
            "Preferred Roblox installation set to Roblox version folders."
        );

        let c = plan_change(Some(&canonical(VF)), None).unwrap();
        assert_eq!(c.new_raw, None, "automatic removes the setting");
        assert_eq!((c.from, c.to), ("versionFolder", "automatic"));
        assert_eq!(
            c.message,
            "Preferred Roblox installation reset to automatic."
        );
    }

    #[test]
    fn an_unreadable_stored_value_is_replaced_and_reported_as_unrecognised_without_echoing_it() {
        for raw in [
            r#"{"v":1,"source":"C:\\Users\\secret\\Roblox.exe"}"#, // invalid
            r#"{"v":2,"note":"SECRET"}"#,                          // newer
            r#"{"v":1,"source":"microsoftStore"}"#,                // unsupported source
            "garbage SECRET",
        ] {
            for target in [None, Some(PH)] {
                let c = plan_change(Some(raw), target).expect("replacing it is a change");
                assert_eq!(c.from, "unrecognised", "{raw}");
                let shown = format!("{} {}", c.message, c.metadata_json()).to_lowercase();
                assert!(
                    !shown.contains("secret") && !shown.contains("users"),
                    "{shown}"
                );
                assert_eq!(c.new_raw, target.map(canonical));
            }
        }
    }

    #[test]
    fn the_log_entry_is_well_formed_for_the_storage_layer() {
        let c = plan_change(None, Some(VF)).unwrap();
        let meta: serde_json::Value = serde_json::from_str(&c.metadata_json()).unwrap();
        assert_eq!(
            meta,
            serde_json::json!({"from": "automatic", "to": "versionFolder"})
        );
        crate::storage::validation::event_type(EVENT_TYPE).unwrap();
        crate::storage::validation::message(&c.message).unwrap();
        crate::storage::validation::setting_key(SETTING_KEY).unwrap();
        // The key is not a "known" setting, so the generic settings commands cannot touch it.
        assert!(
            !crate::storage::settings::KNOWN
                .iter()
                .any(|k| k.key == SETTING_KEY)
        );
    }

    // ---- ordering and duplicates ----

    /// Six version-folder installs covering every tie-break.
    fn tie_set() -> Vec<RobloxInstall> {
        vec![
            vf(A, ROOT, Some(300)),                                          // 0: newest
            vf(B, ROOT, Some(200)),                                          // 1: same date as 2, 3
            vf("version-BBBBBBBBBBBBBBBB", r"E:\Other\Versions", Some(200)), // 2: same name, other case
            vf(B, r"D:\Games\Versions", Some(200)),                          // 3: same name as 1
            vf(C, ROOT, None),                                               // 4: unknown date
            vf(C, r"D:\Games\Versions", None), // 5: unknown, other drive
        ]
    }

    fn tie_order(set: &[RobloxInstall]) -> Vec<RobloxInstall> {
        // newest; then (same date) the name's exact bytes decide 'B' before 'b', then the path;
        // then the unknown dates, again by path.
        [0, 2, 1, 3, 4, 5].map(|i| set[i].clone()).to_vec()
    }

    #[test]
    fn candidates_are_ordered_newest_first_unknown_last_then_by_name_then_path() {
        let set = tie_set();
        let expected = tie_order(&set);
        assert_eq!(candidates(&detection(set, vec![]), VF), expected);
    }

    #[test]
    fn the_result_never_depends_on_the_order_detection_listed_things_in() {
        let set = tie_set();
        let expected = tie_order(&set);
        let all = permutations(&set);
        assert_eq!(all.len(), 720);
        for perm in all {
            assert_eq!(candidates(&detection(perm, vec![]), VF), expected);
        }
        // All-tied top: whichever order they arrive in, the same one wins.
        let tied = vec![set[1].clone(), set[2].clone(), set[3].clone()];
        for perm in permutations(&tied) {
            let r = resolve(&detection(perm, vec![]), StoredPreference::Source(VF));
            assert_eq!(r.chosen.as_ref(), Some(&set[2]));
            assert_eq!(r.candidate_count, 3);
        }
    }

    #[test]
    fn the_same_program_listed_twice_is_counted_once() {
        let first = inst(
            DetectionSource::ProtocolHandler,
            Some(A),
            &format!(r"{ROOT}\{A}\{PLAYER_EXE}"),
            Some(5),
        );
        let again = inst(
            DetectionSource::VersionFolder,
            Some(A),
            &format!(
                "{}/{A}/ROBLOXPLAYERBETA.EXE",
                ROOT.to_lowercase().replace('\\', "/")
            ),
            Some(5),
        );
        let other = vf(B, ROOT, Some(1));
        for perm in permutations(&[first.clone(), again, other]) {
            let r = resolve(&detection(perm, vec![]), StoredPreference::Source(VF));
            assert_eq!(r.candidate_count, 2, "A once, B once");
            assert_eq!(
                r.chosen.as_ref(),
                Some(&first),
                "the first of the duplicates in order wins"
            );
        }
        // Different dates for the same program: the newer record is the one kept.
        let old = inst(
            DetectionSource::VersionFolder,
            Some(A),
            &format!(r"{ROOT}\{A}\{PLAYER_EXE}"),
            Some(1),
        );
        let new = inst(
            DetectionSource::VersionFolder,
            Some(A),
            &format!("{ROOT}/{A}/{PLAYER_EXE}"),
            Some(9),
        );
        for perm in permutations(&[old, new.clone()]) {
            let c = candidates(&detection(perm, vec![]), VF);
            assert_eq!(c, vec![new.clone()]);
        }
        // The link-handler source only ever offers handler installs.
        let r = resolve(
            &detection(vec![first.clone(), vf(B, ROOT, Some(9))], vec![]),
            StoredPreference::Source(PH),
        );
        assert_eq!((r.candidate_count, r.chosen.as_ref()), (1, Some(&first)));
    }

    #[test]
    fn the_folder_the_link_handler_points_to_counts_as_a_version_folder() {
        // The shape of the machine this was built on: a newer folder that the handler points to
        // (listed once, under the handler) and an older one found by the folder scan.
        let env = handler_to(
            machine()
                .with_folders(&[A, B])
                .with_exe(A, file(Some(100)))
                .with_exe(B, file(Some(200))),
            B,
        );
        let d = detect(&env);
        assert_eq!(d.installs.len(), 2, "{:?}", d.installs);
        assert_eq!(d.installs[0].source, DetectionSource::ProtocolHandler);
        let newest = env.exe(B).to_string_lossy().into_owned();

        let folders = resolve(&d, StoredPreference::Source(VF));
        assert_eq!(
            (folders.state, folders.candidate_count),
            (PreferenceState::Available, 2)
        );
        assert_eq!(
            path(&folders),
            Some(newest.as_str()),
            "the newest, not merely the first folder-scan hit"
        );
        let handler = resolve(&d, StoredPreference::Source(PH));
        assert_eq!(
            (handler.state, handler.candidate_count),
            (PreferenceState::Available, 1)
        );
        assert_eq!(path(&handler), Some(newest.as_str()));
        let auto = resolve(&d, StoredPreference::Absent);
        assert_eq!(auto.chosen.as_ref(), d.installs.first());

        // Handler pointing at the OLDER folder: it stays itself; the folders still pick the newest.
        let env = handler_to(
            machine()
                .with_folders(&[A, B])
                .with_exe(A, file(Some(100)))
                .with_exe(B, file(Some(200))),
            A,
        );
        let d = detect(&env);
        let older = env.exe(A).to_string_lossy().into_owned();
        let newer = env.exe(B).to_string_lossy().into_owned();
        assert_eq!(
            path(&resolve(&d, StoredPreference::Source(PH))),
            Some(older.as_str())
        );
        assert_eq!(
            path(&resolve(&d, StoredPreference::Source(VF))),
            Some(newer.as_str())
        );
    }

    #[test]
    fn a_launcher_program_is_not_a_version_folder_game_client() {
        let launcher = format!(r"{ROOT}\{A}\RobloxPlayerLauncher.exe");
        let env = with_handler_text(
            machine().with_folders(&[A]).with_exe(A, file(Some(100))),
            &launcher,
        )
        .with_file(&launcher, file(Some(300)));
        let d = detect(&env);
        assert_eq!(d.installs.len(), 2);
        let folders = resolve(&d, StoredPreference::Source(VF));
        assert_eq!(folders.candidate_count, 1);
        assert_eq!(
            folders.chosen.as_ref().unwrap().source,
            DetectionSource::VersionFolder
        );
        assert_eq!(
            path(&resolve(&d, StoredPreference::Source(PH))),
            Some(launcher.as_str())
        );
    }

    #[test]
    fn resolver_order_agrees_with_detections_own_order() {
        let configs: Vec<(&str, Fake)> = vec![
            (
                "mixed dates and an unknown",
                machine()
                    .with_folders(&[A, B, C])
                    .with_exe(A, file(Some(100)))
                    .with_exe(B, file(Some(300)))
                    .with_exe(C, file(None)),
            ),
            (
                "all tied",
                machine()
                    .with_folders(&[C, A, B])
                    .with_exe(A, file(Some(7)))
                    .with_exe(B, file(Some(7)))
                    .with_exe(C, file(Some(7))),
            ),
            (
                "handler inside the list",
                handler_to(
                    machine()
                        .with_folders(&[A, B, C])
                        .with_exe(A, file(Some(300)))
                        .with_exe(B, file(Some(200)))
                        .with_exe(C, file(Some(100))),
                    C,
                ),
            ),
            (
                "unknown dates only",
                machine()
                    .with_folders(&[B, A])
                    .with_exe(A, file(None))
                    .with_exe(B, file(None)),
            ),
        ];
        for (name, env) in configs {
            let d = detect(&env);
            let by_detection: Vec<&str> = d
                .installs
                .iter()
                .filter(|i| i.source == DetectionSource::VersionFolder)
                .map(|i| i.executable_path.as_str())
                .collect();
            let all = candidates(&d, VF);
            let by_resolver: Vec<&str> = all
                .iter()
                .filter(|i| i.source == DetectionSource::VersionFolder)
                .map(|i| i.executable_path.as_str())
                .collect();
            assert_eq!(by_resolver, by_detection, "{name}");
            // With no handler in play, the newest folder is also detection's primary entry.
            if !d
                .installs
                .iter()
                .any(|i| i.source == DetectionSource::ProtocolHandler)
            {
                assert_eq!(all.first(), d.installs.first(), "{name}");
            }
        }
    }

    // ---- zero, one or many candidates, per source ----

    /// Machines where nothing was found, with what each source should say.
    fn empty_machines() -> Vec<(&'static str, Fake, PreferenceState, PreferenceState)> {
        use PreferenceState::{Inconclusive as Inc, Unavailable as Un};
        let target = format!(r"{ROOT}\{A}\{PLAYER_EXE}");
        let mut no_list = machine();
        no_list
            .dirs
            .insert(PathBuf::from(ROOT), DirListing::Inaccessible);
        let mut partial = machine();
        partial.dirs.insert(
            PathBuf::from(ROOT),
            DirListing::Entries {
                names: vec![],
                complete: false,
            },
        );
        vec![
            // (name, machine, what the link handler says, what the version folders say)
            ("nothing at all", machine(), Un, Un),
            (
                "only stray folders",
                machine().with_folders(&["junk"]),
                Un,
                Un,
            ),
            (
                "folder without a program",
                machine().with_folders(&[A]),
                Un,
                Un,
            ),
            (
                "program is empty",
                machine().with_folders(&[A]).with_exe(A, FileState::Empty),
                Un,
                Un,
            ),
            (
                "program unreadable",
                machine()
                    .with_folders(&[A])
                    .with_exe(A, FileState::Inaccessible),
                Un,
                Inc,
            ),
            ("Versions folder unreadable", no_list, Un, Inc),
            ("Versions folder listing partial", partial, Un, Inc),
            (
                "handler unreadable",
                machine().with_handler(RegistryValue::Inaccessible),
                Inc,
                Un,
            ),
            (
                "handler malformed",
                machine().with_handler(RegistryValue::Malformed),
                Inc,
                Un,
            ),
            (
                "handler text unparseable",
                with_handler_text(machine(), "garbage"),
                Inc,
                Un,
            ),
            (
                "handler opens another program",
                with_handler_text(machine(), r"C:\Windows\notepad.exe"),
                Un,
                Un,
            ),
            (
                "handler target missing",
                with_handler_text(machine(), &target),
                Un,
                Un,
            ),
            (
                "handler target unreadable",
                with_handler_text(machine(), &target).with_file(&target, FileState::Inaccessible),
                Inc,
                Un,
            ),
        ]
    }

    #[test]
    fn zero_candidates_are_unavailable_only_after_a_clean_no_otherwise_inconclusive() {
        for (name, env, handler_says, folders_say) in empty_machines() {
            let d = detect(&env);
            assert!(d.installs.is_empty(), "{name}: {:?}", d.installs);
            for (source, expected) in [(PH, handler_says), (VF, folders_say)] {
                let r = resolve(&d, StoredPreference::Source(source));
                assert_eq!(r.state, expected, "{name} / {source:?}");
                assert_eq!(
                    (r.preferred, r.chosen.is_some(), r.candidate_count),
                    (Some(source), false, 0)
                );
            }
        }
    }

    #[test]
    fn resolution_is_never_more_confident_than_detection() {
        for (name, env, _, _) in empty_machines() {
            let d = detect(&env);
            let states = [PH, VF].map(|s| resolve(&d, StoredPreference::Source(s)).state);
            match d.status {
                // Everything was checked and was a clean "no": both sources may say "unavailable".
                DetectionStatus::NotDetected => {
                    assert_eq!(states, [PreferenceState::Unavailable; 2], "{name}")
                }
                // Detection could not tell, so at least one source must not claim "unavailable".
                DetectionStatus::Inconclusive => assert!(
                    states.contains(&PreferenceState::Inconclusive),
                    "{name}: {states:?}"
                ),
                other => panic!("{name}: unexpected status {other:?}"),
            }
        }
    }

    #[test]
    fn one_or_many_candidates_are_available_and_counted() {
        let one = detect(&machine().with_folders(&[A]).with_exe(A, file(Some(1))));
        let r = resolve(&one, StoredPreference::Source(VF));
        assert_eq!(
            (r.state, r.candidate_count),
            (PreferenceState::Available, 1)
        );

        let many = detect(
            &machine()
                .with_folders(&[A, B, C])
                .with_exe(A, file(Some(1)))
                .with_exe(B, file(Some(3)))
                .with_exe(C, file(Some(2))),
        );
        let r = resolve(&many, StoredPreference::Source(VF));
        assert_eq!(
            (r.state, r.candidate_count),
            (PreferenceState::Available, 3)
        );
        assert_eq!(
            r.chosen.unwrap().version.as_deref(),
            Some(B),
            "the newest of three"
        );
    }

    #[test]
    fn an_unavailable_preference_never_falls_back_to_another_install() {
        // The folders hold a usable install, but the link handler is not set.
        let d = detect(&machine().with_folders(&[A]).with_exe(A, file(Some(1))));
        assert_eq!(d.status, DetectionStatus::Detected);
        let r = resolve(&d, StoredPreference::Source(PH));
        assert_eq!((r.state, r.chosen), (PreferenceState::Unavailable, None));
        // Only "automatic" uses detection's primary entry.
        assert_eq!(
            resolve(&d, StoredPreference::Absent).chosen.as_ref(),
            d.installs.first()
        );
    }

    // ---- the ten probe outcomes ----

    const ALL_OUTCOMES: [ProbeOutcome; 10] = [
        ProbeOutcome::Found,
        ProbeOutcome::NotFound,
        ProbeOutcome::MissingExecutable,
        ProbeOutcome::NotAFile,
        ProbeOutcome::Inaccessible,
        ProbeOutcome::Incomplete,
        ProbeOutcome::Malformed,
        ProbeOutcome::Unrecognised,
        ProbeOutcome::Unsupported,
        ProbeOutcome::Several,
    ];

    /// Exhaustive on purpose: adding an outcome stops this compiling until [`ALL_OUTCOMES`] and the
    /// table below are updated too.
    fn position(o: ProbeOutcome) -> usize {
        match o {
            ProbeOutcome::Found => 0,
            ProbeOutcome::NotFound => 1,
            ProbeOutcome::MissingExecutable => 2,
            ProbeOutcome::NotAFile => 3,
            ProbeOutcome::Inaccessible => 4,
            ProbeOutcome::Incomplete => 5,
            ProbeOutcome::Malformed => 6,
            ProbeOutcome::Unrecognised => 7,
            ProbeOutcome::Unsupported => 8,
            ProbeOutcome::Several => 9,
        }
    }

    #[test]
    fn every_probe_outcome_has_the_agreed_meaning_for_each_source() {
        use Evidence::{Negative as N, Neutral as E, Unresolved as U};
        assert_eq!(ALL_OUTCOMES.map(position), [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        // (outcome, link handler, version folders)
        let table = [
            (ProbeOutcome::Found, U, U),
            (ProbeOutcome::NotFound, N, N),
            (ProbeOutcome::MissingExecutable, N, N),
            (ProbeOutcome::NotAFile, N, N),
            (ProbeOutcome::Inaccessible, U, U),
            (ProbeOutcome::Incomplete, U, U),
            (ProbeOutcome::Malformed, U, E), // folders: only means some names were ignored
            (ProbeOutcome::Unrecognised, N, E),
            (ProbeOutcome::Unsupported, E, E),
            (ProbeOutcome::Several, U, U),
        ];
        assert_eq!(table.len(), ALL_OUTCOMES.len());
        for (i, (outcome, handler, folders)) in table.into_iter().enumerate() {
            assert_eq!(outcome, ALL_OUTCOMES[i]);
            assert_eq!(evidence(PH, outcome), handler, "{outcome:?} / link handler");
            assert_eq!(
                evidence(VF, outcome),
                folders,
                "{outcome:?} / version folders"
            );
        }
    }

    #[test]
    fn findings_combine_into_a_state_as_agreed() {
        use ProbeOutcome as O;
        let state = |source: PreferredSource, outcomes: &[ProbeOutcome]| {
            let findings = outcomes
                .iter()
                .map(|o| finding(source.detection_source(), *o))
                .collect();
            resolve(
                &detection(vec![], findings),
                StoredPreference::Source(source),
            )
            .state
        };
        for source in [PH, VF] {
            // One finding alone: a clean "no" is unavailable; a doubt or no information is inconclusive.
            for o in ALL_OUTCOMES {
                let expected = match evidence(source, o) {
                    Evidence::Negative => PreferenceState::Unavailable,
                    Evidence::Unresolved | Evidence::Neutral => PreferenceState::Inconclusive,
                };
                assert_eq!(state(source, &[o]), expected, "{source:?} / {o:?}");
            }
            // Any doubt beats any number of clean "no"s, in either order.
            assert_eq!(
                state(source, &[O::NotFound, O::Inaccessible]),
                PreferenceState::Inconclusive
            );
            assert_eq!(
                state(source, &[O::Inaccessible, O::NotFound]),
                PreferenceState::Inconclusive
            );
            // Neutral findings neither help nor hurt a clean "no".
            assert_eq!(
                state(source, &[O::Unsupported, O::NotFound]),
                PreferenceState::Unavailable
            );
            // No findings at all is no basis for "unavailable".
            assert_eq!(state(source, &[]), PreferenceState::Inconclusive);
        }
        // Only the preferred source's own findings count.
        let other = detection(
            vec![],
            vec![finding(DetectionSource::VersionFolder, O::NotFound)],
        );
        assert_eq!(
            resolve(&other, StoredPreference::Source(PH)).state,
            PreferenceState::Inconclusive
        );
        // The version-folder "ignored names" note does not turn a clean "no" into doubt.
        assert_eq!(
            state(VF, &[O::Malformed, O::NotFound]),
            PreferenceState::Unavailable
        );
    }

    /// Twelve version folders, which is more than detection lists individually (it shows ten
    /// problems and summarises the rest as one `Several` finding).
    fn twelve_folders() -> Vec<String> {
        (0..12).map(|i| format!("version-{i:016x}")).collect()
    }

    #[test]
    fn hidden_problems_reach_the_resolver_as_several_and_are_treated_cautiously() {
        let names = twelve_folders();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let has_several = |d: &RobloxDetection| {
            d.findings.iter().any(|f| {
                f.source == DetectionSource::VersionFolder && f.outcome == ProbeOutcome::Several
            })
        };

        // All twelve lack a program, so every problem is a clean "no" and detection says "not
        // detected". Ten are listed, two are summarised. The resolver cannot see what the summary
        // hides, so it declines to say "unavailable": cautious, never the other way round.
        let d = detect(&machine().with_folders(&refs));
        assert!(has_several(&d), "{:?}", d.findings);
        assert_eq!(d.status, DetectionStatus::NotDetected);
        assert_eq!(
            resolve(&d, StoredPreference::Source(VF)).state,
            PreferenceState::Inconclusive
        );
        // The link handler's own evidence is clean and unaffected.
        assert_eq!(
            resolve(&d, StoredPreference::Source(PH)).state,
            PreferenceState::Unavailable
        );

        // One of the two hidden folders is unreadable: detection itself now cannot tell, and the
        // resolver agrees. The handler source is still a conclusive "no".
        let env = machine()
            .with_folders(&refs)
            .with_exe(&names[11], FileState::Inaccessible);
        let d = detect(&env);
        assert!(has_several(&d), "{:?}", d.findings);
        assert_eq!(d.status, DetectionStatus::Inconclusive);
        assert_eq!(
            resolve(&d, StoredPreference::Source(VF)).state,
            PreferenceState::Inconclusive
        );
        assert_eq!(
            resolve(&d, StoredPreference::Source(PH)).state,
            PreferenceState::Unavailable
        );
    }

    // ---- the other states, and independence from detection ----

    #[test]
    fn automatic_uses_the_primary_install_and_reports_no_source() {
        let d = detect(&machine().with_folders(&[A]).with_exe(A, file(Some(1))));
        let r = resolve(&d, StoredPreference::Absent);
        assert_eq!(r.state, PreferenceState::Automatic);
        assert_eq!((r.preferred, r.candidate_count, r.reason), (None, 0, None));
        assert_eq!(r.chosen.as_ref(), d.installs.first());
        assert!(r.chosen.is_some());

        let none = resolve(&detect(&machine()), StoredPreference::Absent);
        assert_eq!(
            (none.state, none.chosen),
            (PreferenceState::Automatic, None)
        );
        let mut off = Fake::windows();
        off.supported = false;
        let r = resolve(&detect(&off), StoredPreference::Absent);
        assert_eq!((r.state, r.chosen), (PreferenceState::Automatic, None));
    }

    #[test]
    fn values_that_cannot_be_used_have_their_own_states() {
        let d = detect(&machine().with_folders(&[A]).with_exe(A, file(Some(1))));
        let invalid = resolve(&d, StoredPreference::Invalid);
        assert_eq!(
            (
                invalid.state,
                invalid.preferred,
                invalid.chosen.clone(),
                invalid.reason
            ),
            (PreferenceState::Invalid, None, None, None)
        );
        let newer = resolve(&d, StoredPreference::NewerFormat);
        assert_eq!(
            (newer.state, newer.reason),
            (
                PreferenceState::Unsupported,
                Some(PreferenceReason::NewerFormat)
            )
        );
        let source = resolve(&d, StoredPreference::UnsupportedSource);
        assert_eq!(
            (source.state, source.reason),
            (
                PreferenceState::Unsupported,
                Some(PreferenceReason::UnsupportedSource)
            )
        );
        for r in [&invalid, &newer, &source] {
            assert_eq!(
                (r.chosen.as_ref(), r.candidate_count),
                (None, 0),
                "no install is offered"
            );
        }
    }

    #[test]
    fn a_source_preference_on_an_unsupported_system_is_unsupported_not_unavailable() {
        let mut off = Fake::windows();
        off.supported = false;
        let d = detect(&off);
        assert_eq!(d.status, DetectionStatus::Unsupported);
        for s in [PH, VF] {
            let r = resolve(&d, StoredPreference::Source(s));
            assert_eq!(r.state, PreferenceState::Unsupported);
            assert_eq!(
                (r.reason, r.preferred),
                (Some(PreferenceReason::UnsupportedPlatform), Some(s))
            );
        }
        // A value this app cannot read is a data problem on any system, and is reported as one.
        assert_eq!(
            resolve(&d, StoredPreference::Invalid).state,
            PreferenceState::Invalid
        );
    }

    #[test]
    fn resolving_never_changes_what_detection_reported() {
        let machines = [
            detect(&machine().with_folders(&[A, B]).with_exe(A, file(Some(1)))),
            detect(&machine()),
            detect(&machine().with_handler(RegistryValue::Inaccessible)),
        ];
        let stored = [
            StoredPreference::Absent,
            StoredPreference::Source(PH),
            StoredPreference::Source(VF),
            StoredPreference::UnsupportedSource,
            StoredPreference::NewerFormat,
            StoredPreference::Invalid,
        ];
        for d in &machines {
            let before = d.clone();
            for s in stored {
                let _ = resolve(d, s);
                assert_eq!(d, &before);
            }
        }
    }

    #[test]
    fn the_resolution_serialises_with_the_names_the_screen_will_use() {
        let d = detect(
            &machine()
                .with_folders(&[A, B])
                .with_exe(A, file(Some(1)))
                .with_exe(B, file(Some(2))),
        );
        let json = serde_json::to_value(resolve(&d, StoredPreference::Source(VF))).unwrap();
        assert_eq!(json["state"], "available");
        assert_eq!(json["preferred"], "versionFolder");
        assert_eq!(json["candidateCount"], 2);
        assert_eq!(json["reason"], serde_json::Value::Null);
        assert_eq!(json["chosen"]["version"], B);
        let json = serde_json::to_value(resolve(&d, StoredPreference::NewerFormat)).unwrap();
        assert_eq!(json["state"], "unsupported");
        assert_eq!(json["reason"], "newerFormat");
        assert_eq!(json["chosen"], serde_json::Value::Null);
    }

    // ---- which sources can be chosen right now ----

    /// Only the link-handler source resolves: it points at a program outside any version folder.
    fn handler_only() -> Fake {
        let launcher = r"C:\Other\RobloxPlayerLauncher.exe";
        with_handler_text(machine(), launcher).with_file(launcher, file(Some(1)))
    }

    /// Both sources resolve: three folders, and the link handler points at the middle one.
    fn both_sources() -> Fake {
        handler_to(
            machine()
                .with_folders(&[A, B, C])
                .with_exe(A, file(Some(100)))
                .with_exe(B, file(Some(200)))
                .with_exe(C, file(Some(300))),
            B,
        )
    }

    #[test]
    fn available_sources_are_exactly_the_sources_that_resolve_and_in_a_fixed_order() {
        let mut unsupported = Fake::windows();
        unsupported.supported = false;
        let mut machines: Vec<(&str, RobloxDetection)> = empty_machines()
            .into_iter()
            .map(|(name, env, _, _)| (name, detect(&env)))
            .collect();
        machines.extend([
            ("both", detect(&both_sources())),
            (
                "folders only",
                detect(&machine().with_folders(&[A]).with_exe(A, file(Some(1)))),
            ),
            ("handler only", detect(&handler_only())),
            ("unsupported", detect(&unsupported)),
            // One source cannot be checked, the other is fine.
            (
                "folders, handler unreadable",
                detect(
                    &machine()
                        .with_folders(&[A])
                        .with_exe(A, file(Some(1)))
                        .with_handler(RegistryValue::Inaccessible),
                ),
            ),
            ("handler, Versions unreadable", {
                let mut env = handler_only();
                env.dirs
                    .insert(PathBuf::from(ROOT), DirListing::Inaccessible);
                detect(&env)
            }),
        ]);

        // The invariant: listed if and only if resolving that source says "available".
        for (name, d) in &machines {
            let by_resolving: Vec<PreferredSource> = [PH, VF]
                .into_iter()
                .filter(|s| {
                    resolve(d, StoredPreference::Source(*s)).state == PreferenceState::Available
                })
                .collect();
            assert_eq!(available_sources(d), by_resolving, "{name}");
        }

        // And the concrete answers, so the invariant cannot hide a wrong-on-both-sides rule.
        let listed =
            |name: &str| available_sources(&machines.iter().find(|(n, _)| *n == name).unwrap().1);
        let nothing: Vec<PreferredSource> = Vec::new();
        assert_eq!(listed("both"), [PH, VF], "fixed order: link handler first");
        assert_eq!(listed("folders only"), [VF]);
        assert_eq!(listed("handler only"), [PH]);
        assert_eq!(listed("unsupported"), nothing);
        assert_eq!(listed("nothing at all"), nothing);
        // A source that could not be checked is not offered.
        assert_eq!(listed("Versions folder unreadable"), nothing);
        assert_eq!(listed("handler unreadable"), nothing);
        // ...and one unreadable source does not suppress the other, usable one.
        assert_eq!(listed("folders, handler unreadable"), [VF]);
        assert_eq!(listed("handler, Versions unreadable"), [PH]);
    }

    #[test]
    fn available_sources_do_not_depend_on_the_order_detection_listed_things_in() {
        let d = detect(&both_sources());
        assert_eq!(d.installs.len(), 3);
        for perm in permutations(&d.installs) {
            let shuffled = RobloxDetection {
                installs: perm,
                ..d.clone()
            };
            assert_eq!(available_sources(&shuffled), [PH, VF]);
        }
    }
}
