//! Text templates for notifications; trigger conditions stay outside this module.

use serde::Deserialize;
use std::{collections::HashMap, sync::LazyLock};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationTemplate {
    pub title: String,
    pub content: String,
    pub notification_type: String,
}

impl NotificationTemplate {
    pub fn format_content(&self, values: &[(&str, &str)]) -> Result<String, &'static str> {
        let mut content = self.content.clone();

        // Keep the stored templates intact while filling this notifications's values.
        for &(name, value) in values {
            content = content.replace(&format!("{{{name}}}"), value);
        }

        // Braces are reserved for placeholders in notification content.
        if content.contains('{') || content.contains('}') {
            return Err("Unresolved or malformed notification placeholder");
        }

        Ok(content)
    }
}

pub fn load_templates()
-> Result<&'static HashMap<String, NotificationTemplate>, &'static serde_json::Error> {
    static TEMPLATES: LazyLock<Result<HashMap<String, NotificationTemplate>, serde_json::Error>> =
        LazyLock::new(|| {
            // Parse on first access and reuse the catalog for subsequent notifications.
            serde_json::from_str(include_str!("../../resources/notifications.json"))
        });

    TEMPLATES.as_ref()
}
