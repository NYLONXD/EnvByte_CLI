//! `envbyte audit` - the server-side record of who changed what.

use colored::Colorize;

use crate::{commands::context::ProjectContext, core::api::projects, ui};

pub async fn show() -> Result<(), String> {
    let context = ProjectContext::load().await?;
    let events = projects::audit(&context.session, &context.project_id).await?;
    if events.is_empty() {
        ui::note("No audit events recorded yet.");
        return Ok(());
    }
    ui::heading(&format!(
        "Audit log for {}",
        context.config.qualified_name()
    ));
    for event in events {
        let when = event["created_at"].as_str().unwrap_or("");
        let actor = event["actor"].as_str().unwrap_or("system");
        let action = event["action"].as_str().unwrap_or("unknown");
        println!(
            "  {:<22} {:<14} {:<26} {}",
            when.dimmed(),
            actor,
            action.cyan(),
            compact(&event["metadata"]).dimmed()
        );
    }
    Ok(())
}

/// Renders the metadata object as `key=value` pairs rather than raw JSON.
fn compact(metadata: &serde_json::Value) -> String {
    match metadata.as_object() {
        Some(map) if !map.is_empty() => map
            .iter()
            .map(|(key, value)| {
                let rendered = value
                    .as_str()
                    .map(String::from)
                    .unwrap_or_else(|| value.to_string());
                format!("{key}={rendered}")
            })
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_metadata_without_json_punctuation() {
        let metadata = serde_json::json!({ "filename": ".env", "key_version": 2 });
        let rendered = compact(&metadata);
        assert!(rendered.contains("filename=.env"), "{rendered}");
        assert!(rendered.contains("key_version=2"), "{rendered}");
        assert!(!rendered.contains('"'), "{rendered}");
    }

    #[test]
    fn renders_empty_metadata_as_nothing() {
        assert_eq!(compact(&serde_json::json!({})), "");
        assert_eq!(compact(&serde_json::Value::Null), "");
    }
}
