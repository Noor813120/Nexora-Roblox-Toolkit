//! The one error type every IPC command returns.
//!
//! It serializes to `{ "code": "...", "message": "..." }`, so the frontend can branch on a stable
//! `code` and show a human `message`, and never has to parse error strings. Internal details
//! (paths, handles, OS error text) stay in the logs, not in the message.

use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// The request failed validation before anything ran.
    #[error("invalid request: {0}")]
    Invalid(String),
    /// The thing asked for does not exist.
    #[error("not found")]
    NotFound,
    /// The request is well formed but clashes with existing data (duplicate name, default profile).
    #[error("{0}")]
    Conflict(String),
    /// The database could not be opened at startup, so nothing that needs it can run.
    #[error("the database is not available")]
    Unavailable,
    /// Something went wrong inside the app; the cause is in the log.
    #[error("internal error")]
    Internal,
}

impl AppError {
    /// Stable machine-readable code for the frontend.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid(_) => "invalid",
            Self::NotFound => "not_found",
            Self::Conflict(_) => "conflict",
            Self::Unavailable => "unavailable",
            Self::Internal => "internal",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("AppError", 2)?;
        st.serialize_field("code", self.code())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_code_and_message() {
        let v = serde_json::to_value(AppError::Invalid("name is empty".into())).unwrap();
        assert_eq!(v["code"], "invalid");
        assert_eq!(v["message"], "invalid request: name is empty");
    }

    #[test]
    fn internal_error_hides_details() {
        let v = serde_json::to_value(AppError::Internal).unwrap();
        assert_eq!(v["code"], "internal");
        assert_eq!(v["message"], "internal error");
    }

    #[test]
    fn every_variant_has_a_distinct_code() {
        let codes = [
            AppError::Invalid(String::new()).code(),
            AppError::NotFound.code(),
            AppError::Conflict(String::new()).code(),
            AppError::Unavailable.code(),
            AppError::Internal.code(),
        ];
        let mut sorted = codes.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), codes.len());
    }
}
