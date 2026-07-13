use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub provider: String,
    pub provider_display_name: String,
    pub session_id: String,
    pub title: Option<String>,
    pub friendly_name: Option<String>,
    pub collection: Option<String>,
    pub collection_color: Option<String>,
    pub note: Option<String>,
    pub tags: Vec<String>,
    pub display_name: String,
    pub first_user_input: Option<String>,
    pub last_user_input: Option<String>,
    pub last_message_preview: Option<String>,
    pub last_message_role: Option<String>,
    pub working_directory: Option<String>,
    pub discovered_repository: Option<String>,
    pub discovered_branch: Option<String>,
    pub discovered_at: Option<i64>,
    pub resume_command: String,
    pub last_modified: Option<i64>,
    pub last_resumed: Option<i64>,
    pub can_delete: bool,
    pub can_resume: bool,
    pub is_hidden: bool,
    pub is_pinned: bool,
    pub is_favorite_project: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSearchResult {
    pub provider: String,
    pub session_id: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessage {
    pub role: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionHistory {
    pub provider: String,
    pub session_id: String,
    pub messages: Vec<SessionMessage>,
    pub unreadable_lines: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub id: String,
    pub display_name: String,
    pub executable: String,
    pub session_store: String,
    pub available: bool,
    pub sessions_path_exists: bool,
    pub delete_supported: bool,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String,
    pub terminal_executable: Option<String>,
    pub provider_filter: String,
    pub show_hidden_sessions: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            terminal_executable: None,
            provider_filter: "all".to_string(),
            show_hidden_sessions: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
    pub action: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallResult {
    pub message: String,
    pub app_removal_attempted: bool,
    pub app_removed: bool,
}
