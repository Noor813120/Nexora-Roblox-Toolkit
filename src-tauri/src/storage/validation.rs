//! Input rules shared by the storage modules. Every value is checked here, in Rust, before it
//! reaches SQL; the schema's CHECK constraints are a second line of defence, not the first.
//!
//! Lengths are counted in characters (matching SQLite's `length()`), not bytes.

use super::errors::{Result, StorageError};

pub const NAME_MAX: usize = 64;
pub const DESCRIPTION_MAX: usize = 500;
pub const SETTING_KEY_MAX: usize = 128;
pub const SETTING_VALUE_MAX: usize = 8192;
pub const EVENT_TYPE_MAX: usize = 64;
pub const MESSAGE_MAX: usize = 2000;
pub const CONFIG_JSON_MAX: usize = 65_536;
pub const METADATA_JSON_MAX: usize = 16_384;

fn invalid<T>(msg: impl Into<String>) -> Result<T> {
    Err(StorageError::Invalid(msg.into()))
}

fn has_control(s: &str) -> bool {
    s.chars().any(char::is_control)
}

/// A display name: trimmed, 1..=64 characters, no control characters. Returns the trimmed value.
pub fn name(raw: &str, what: &str) -> Result<String> {
    let v = raw.trim();
    if v.is_empty() {
        return invalid(format!("{what} name cannot be empty"));
    }
    if v.chars().count() > NAME_MAX {
        return invalid(format!("{what} name is longer than {NAME_MAX} characters"));
    }
    if has_control(v) {
        return invalid(format!("{what} name contains control characters"));
    }
    Ok(v.to_owned())
}

/// A free-text description: up to 500 characters; newlines and tabs allowed, other control
/// characters not.
pub fn description(raw: &str) -> Result<String> {
    let v = raw.trim();
    if v.chars().count() > DESCRIPTION_MAX {
        return invalid(format!(
            "description is longer than {DESCRIPTION_MAX} characters"
        ));
    }
    if v.chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        return invalid("description contains control characters");
    }
    Ok(v.to_owned())
}

/// Configuration text must be a JSON *object* (not a number, array or `null`) of limited size.
/// Returns the text unchanged: the caller's formatting is kept.
pub fn json_object(raw: &str, what: &str, max: usize) -> Result<String> {
    if raw.chars().count() > max {
        return invalid(format!("{what} is larger than {max} characters"));
    }
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(serde_json::Value::Object(_)) => Ok(raw.to_owned()),
        Ok(_) => invalid(format!("{what} must be a JSON object")),
        Err(_) => invalid(format!("{what} is not valid JSON")),
    }
}

/// Setting keys look like `activity.retention_days`: lowercase letters, digits, `.`, `_`, `-`;
/// they start with a letter.
pub fn setting_key(raw: &str) -> Result<&str> {
    let ok = !raw.is_empty()
        && raw.chars().count() <= SETTING_KEY_MAX
        && raw.starts_with(|c: char| c.is_ascii_lowercase())
        && raw
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    if ok {
        Ok(raw)
    } else {
        invalid(
            "setting key must be lowercase letters, digits, '.', '_' or '-' and start with a letter",
        )
    }
}

pub fn setting_value(raw: &str) -> Result<&str> {
    if raw.chars().count() > SETTING_VALUE_MAX {
        return invalid(format!(
            "setting value is longer than {SETTING_VALUE_MAX} characters"
        ));
    }
    if raw.contains('\0') {
        return invalid("setting value contains a NUL character");
    }
    Ok(raw)
}

/// Event types are dotted lowercase identifiers, e.g. `database.backup_created`.
pub fn event_type(raw: &str) -> Result<&str> {
    let ok = !raw.is_empty()
        && raw.chars().count() <= EVENT_TYPE_MAX
        && raw.starts_with(|c: char| c.is_ascii_lowercase())
        && raw
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    if ok {
        Ok(raw)
    } else {
        invalid(
            "event type must be lowercase letters, digits, '.', '_' or '-' and start with a letter",
        )
    }
}

pub fn message(raw: &str) -> Result<String> {
    let v = raw.trim();
    if v.is_empty() {
        return invalid("message cannot be empty");
    }
    if v.chars().count() > MESSAGE_MAX {
        return invalid(format!("message is longer than {MESSAGE_MAX} characters"));
    }
    if v.chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        return invalid("message contains control characters");
    }
    Ok(v.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_trimmed_and_bounded() {
        assert_eq!(name("  Main  ", "profile").unwrap(), "Main");
        assert!(name("   ", "profile").is_err());
        assert!(name(&"x".repeat(65), "profile").is_err());
        assert!(name(&"x".repeat(64), "profile").is_ok());
        assert!(name("bad\nname", "profile").is_err());
        // 64 multi-byte characters are fine: the limit counts characters, not bytes.
        assert!(name(&"é".repeat(64), "profile").is_ok());
    }

    #[test]
    fn description_allows_newlines_but_not_other_control_characters() {
        assert!(description("line one\nline two").is_ok());
        assert!(description("bell\u{7}").is_err());
        assert!(description(&"x".repeat(501)).is_err());
        assert_eq!(description("").unwrap(), "");
    }

    #[test]
    fn json_must_be_an_object() {
        assert!(json_object("{}", "configuration", 100).is_ok());
        assert!(json_object(r#"{"a": [1,2]}"#, "configuration", 100).is_ok());
        for bad in ["[]", "null", "3", "\"x\"", "{", "", "{'a':1}"] {
            assert!(json_object(bad, "configuration", 100).is_err(), "{bad:?}");
        }
        assert!(json_object(r#"{"a":"long long"}"#, "configuration", 5).is_err());
    }

    #[test]
    fn setting_keys_follow_the_pattern() {
        for ok in ["a", "activity.retention_days", "ui.theme-2"] {
            assert!(setting_key(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "1abc",
            "Upper",
            "has space",
            "semi;colon",
            "quote'",
            ".dot",
            "é",
        ] {
            assert!(setting_key(bad).is_err(), "{bad:?}");
        }
        assert!(setting_key(&"a".repeat(129)).is_err());
    }

    #[test]
    fn setting_values_reject_nul_and_oversize() {
        assert!(setting_value("fine").is_ok());
        assert!(setting_value("a\0b").is_err());
        assert!(setting_value(&"x".repeat(8193)).is_err());
    }

    #[test]
    fn event_types_and_messages() {
        assert!(event_type("database.backup_created").is_ok());
        assert!(event_type("Bad").is_err());
        assert!(event_type("").is_err());
        assert_eq!(message("  hi  ").unwrap(), "hi");
        assert!(message("  ").is_err());
        assert!(message(&"m".repeat(2001)).is_err());
    }
}
