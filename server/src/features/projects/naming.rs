//! Project naming rules.
//!
//! Names are unique within one owner's account, not across the installation.
//! A global namespace meant the first account to take a common name held it
//! for everyone, and made the set of existing names probeable.

use crate::error::ApiError;

pub fn validate_project_name(value: &str) -> Result<(), ApiError> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(ApiError::BadRequest("invalid project name".to_string()));
    }
    Ok(())
}

/// Turns a per-owner name collision into a message that says what to do,
/// rather than the generic "resource already exists".
pub fn duplicate_name_error(error: sqlx::Error, name: &str) -> ApiError {
    match &error {
        sqlx::Error::Database(db) if db.is_unique_violation() => ApiError::Conflict(format!(
            "you already own a project named '{name}'; names only need to be unique within \
             your own account, so a colleague may still have one by that name"
        )),
        _ => ApiError::from(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_project_names() {
        assert!(validate_project_name("api-production_2").is_ok());
        assert!(validate_project_name("bad project").is_err());
        assert!(validate_project_name("").is_err());
        assert!(validate_project_name(&"a".repeat(65)).is_err());
    }
}
