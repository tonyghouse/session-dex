mod claude;
mod codex;

use crate::models::ProviderStatus;
use crate::terminal;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const PREVIEW_LINE_LIMIT: usize = 3;
const PREVIEW_CHAR_LIMIT: usize = 420;
const SEARCH_LINE_CHAR_LIMIT: usize = 260;
const SESSION_CARD_CACHE_LIMIT: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileFingerprint {
    length: u64,
    modified: SystemTime,
}

#[derive(Debug, Clone, Default)]
struct SessionCardMetadata {
    first_user_input: Option<String>,
    last_user_input: Option<String>,
    last_message_preview: Option<String>,
    last_message_role: Option<String>,
    working_directory: Option<String>,
}

#[derive(Debug, Clone)]
struct CachedSessionCard {
    fingerprint: FileFingerprint,
    metadata: SessionCardMetadata,
}

static SESSION_CARD_CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedSessionCard>>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct ProviderSession {
    pub session_id: String,
    pub title: Option<String>,
    pub first_user_input: Option<String>,
    pub last_user_input: Option<String>,
    pub last_message_preview: Option<String>,
    pub last_message_role: Option<String>,
    pub working_directory: Option<String>,
    pub last_modified: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ProviderSessionSearchMatch {
    pub session_id: String,
    pub snippet: String,
}

#[derive(Debug, Clone)]
pub struct ProviderChatMessage {
    pub role: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct ProviderSessionHistory {
    pub messages: Vec<ProviderChatMessage>,
    pub unreadable_lines: usize,
}

#[derive(Debug, Clone)]
pub struct ResumeCommand {
    pub program: &'static str,
    pub args: Vec<String>,
    pub working_directory: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct ProviderCapabilities {
    pub discover_sessions: bool,
    pub search_sessions: bool,
    pub read_history: bool,
    pub resume_sessions: bool,
    pub delete_sessions: bool,
    pub watch_sessions: bool,
}

impl ProviderCapabilities {
    pub const LOCAL_SESSION_FILES: Self = Self {
        discover_sessions: true,
        search_sessions: true,
        read_history: true,
        resume_sessions: true,
        delete_sessions: false,
        watch_sessions: false,
    };

    pub fn labels(&self) -> Vec<&'static str> {
        let mut labels = Vec::new();

        if self.discover_sessions {
            labels.push("discover");
        }

        if self.search_sessions {
            labels.push("search");
        }

        if self.read_history {
            labels.push("history");
        }

        if self.resume_sessions {
            labels.push("resume");
        }

        if self.delete_sessions {
            labels.push("delete");
        }

        if self.watch_sessions {
            labels.push("watch");
        }

        labels
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProviderDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub executable: &'static str,
    pub session_store: &'static str,
    pub capabilities: ProviderCapabilities,
}

pub type ProviderFactory = fn() -> Box<dyn SessionProvider>;

#[derive(Clone, Copy)]
pub struct ProviderRegistration {
    pub descriptor: &'static ProviderDescriptor,
    pub factory: ProviderFactory,
}

impl ProviderRegistration {
    pub fn provider(&self) -> Box<dyn SessionProvider> {
        (self.factory)()
    }
}

#[derive(Clone, Copy)]
pub struct ProviderRegistry {
    registrations: &'static [ProviderRegistration],
}

impl ProviderRegistry {
    pub fn registrations(&self) -> &'static [ProviderRegistration] {
        self.registrations
    }

    pub fn providers(&self) -> Vec<Box<dyn SessionProvider>> {
        self.registrations()
            .iter()
            .map(ProviderRegistration::provider)
            .collect()
    }

    pub fn by_id(&self, id: &str) -> Option<Box<dyn SessionProvider>> {
        self.registrations()
            .iter()
            .find(|registration| registration.descriptor.id == id)
            .map(ProviderRegistration::provider)
    }

    pub fn statuses(&self) -> Vec<ProviderStatus> {
        self.registrations()
            .iter()
            .map(|registration| {
                let provider = registration.provider();
                let descriptor = registration.descriptor;
                let capabilities = provider.capabilities();

                ProviderStatus {
                    id: descriptor.id.to_string(),
                    display_name: descriptor.display_name.to_string(),
                    executable: descriptor.executable.to_string(),
                    session_store: descriptor.session_store.to_string(),
                    available: terminal::command_available(descriptor.executable),
                    sessions_path_exists: provider
                        .sessions_root()
                        .as_deref()
                        .is_some_and(Path::exists),
                    delete_supported: provider.supports_delete(),
                    capabilities: capabilities
                        .labels()
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                }
            })
            .collect()
    }
}

static PROVIDER_REGISTRATIONS: &[ProviderRegistration] =
    &[codex::REGISTRATION, claude::REGISTRATION];
static PROVIDER_REGISTRY: ProviderRegistry = ProviderRegistry {
    registrations: PROVIDER_REGISTRATIONS,
};

pub fn registry() -> &'static ProviderRegistry {
    &PROVIDER_REGISTRY
}

/// Stable internal contract for local AI CLI providers.
///
/// Providers must never mutate provider-owned history during discovery,
/// search, history reads, or resume command generation. Optional destructive
/// operations belong behind explicit capabilities.
pub trait SessionProvider {
    fn descriptor(&self) -> &'static ProviderDescriptor;
    fn sessions_root(&self) -> Option<PathBuf>;
    fn list_sessions(&self) -> Result<Vec<ProviderSession>, String>;
    fn search_sessions(&self, query: &str) -> Result<Vec<ProviderSessionSearchMatch>, String>;
    fn session_history(&self, session_id: &str) -> Result<ProviderSessionHistory, String>;
    fn resume_command(&self, session_id: &str, working_directory: Option<&str>) -> ResumeCommand;

    fn id(&self) -> &'static str {
        self.descriptor().id
    }

    fn display_name(&self) -> &'static str {
        self.descriptor().display_name
    }

    fn executable(&self) -> &'static str {
        self.descriptor().executable
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.descriptor().capabilities
    }

    fn supports_delete(&self) -> bool {
        self.capabilities().delete_sessions
    }

    fn delete_session(&self, _session_id: &str) -> Result<(), String> {
        Err(format!("{} does not support deletion", self.display_name()))
    }
}

pub fn all() -> Vec<Box<dyn SessionProvider>> {
    registry().providers()
}

pub fn by_id(id: &str) -> Option<Box<dyn SessionProvider>> {
    registry().by_id(id)
}

pub fn statuses() -> Vec<ProviderStatus> {
    registry().statuses()
}

#[cfg(not(target_os = "windows"))]
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(target_os = "windows")]
fn home_dir() -> Option<PathBuf> {
    if let Some(user_profile) = std::env::var_os("USERPROFILE") {
        return Some(PathBuf::from(user_profile));
    }

    match (std::env::var_os("HOMEDRIVE"), std::env::var_os("HOMEPATH")) {
        (Some(drive), Some(path)) => {
            let mut home = drive;
            home.push(path);
            Some(PathBuf::from(home))
        }
        _ => std::env::var_os("HOME").map(PathBuf::from),
    }
}

fn collect_jsonl_files(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_jsonl_files_inner(root, max_depth, 0, &mut files);
    files
}

fn collect_jsonl_files_inner(
    directory: &Path,
    max_depth: usize,
    depth: usize,
    files: &mut Vec<PathBuf>,
) {
    if depth > max_depth {
        return;
    }

    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            collect_jsonl_files_inner(&path, max_depth, depth + 1, files);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "jsonl")
        {
            files.push(path);
        }
    }
}

fn modified_seconds(path: &Path) -> Option<i64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    let duration = modified.duration_since(UNIX_EPOCH).ok()?;
    Some(duration.as_secs() as i64)
}

fn sort_recent_first(sessions: &mut [ProviderSession]) {
    sessions.sort_by(|left, right| right.last_modified.cmp(&left.last_modified));
}

fn chat_history_match(path: &Path, query: &str) -> Option<String> {
    let query = query.trim().to_lowercase();

    if query.is_empty() {
        return None;
    }

    let Ok(file) = File::open(path) else {
        return None;
    };

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };

        for text in extract_chat_message_texts(&value) {
            if let Some(snippet) = format_search_snippet(&text, &query) {
                return Some(snippet);
            }
        }
    }

    None
}

fn read_session_history(path: &Path) -> Result<ProviderSessionHistory, String> {
    let file = File::open(path).map_err(|err| {
        format!(
            "Failed to open session history at {}: {err}",
            path.display()
        )
    })?;
    let mut messages = Vec::new();
    let mut unreadable_lines = 0;

    for line in BufReader::new(file).lines() {
        let Ok(line) = line else {
            unreadable_lines += 1;
            continue;
        };

        if line.trim().is_empty() {
            continue;
        }

        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            unreadable_lines += 1;
            continue;
        };

        messages.extend(extract_chat_messages(&value));
    }

    Ok(ProviderSessionHistory {
        messages,
        unreadable_lines,
    })
}

fn session_card_previews(
    path: &Path,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let metadata = session_card_metadata(path);

    (
        metadata.first_user_input,
        metadata.last_user_input,
        metadata.last_message_preview,
        metadata.last_message_role,
    )
}

fn session_working_directory(path: &Path) -> Option<String> {
    session_card_metadata(path).working_directory
}

fn session_card_metadata(path: &Path) -> SessionCardMetadata {
    let fingerprint = file_fingerprint(path);
    let cache = SESSION_CARD_CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    if let Some(fingerprint) = fingerprint.as_ref() {
        if let Ok(cache) = cache.lock() {
            if let Some(cached) = cache.get(path) {
                if &cached.fingerprint == fingerprint {
                    return cached.metadata.clone();
                }
            }
        }
    }

    let metadata = read_session_card_metadata(path);

    if let Some(fingerprint) = fingerprint {
        if let Ok(mut cache) = cache.lock() {
            if cache.len() >= SESSION_CARD_CACHE_LIMIT && !cache.contains_key(path) {
                let remove_count = (SESSION_CARD_CACHE_LIMIT / 4).max(1);
                let expired_paths = cache.keys().take(remove_count).cloned().collect::<Vec<_>>();

                for expired_path in expired_paths {
                    cache.remove(&expired_path);
                }
            }

            cache.insert(
                path.to_path_buf(),
                CachedSessionCard {
                    fingerprint,
                    metadata: metadata.clone(),
                },
            );
        }
    }

    metadata
}

fn file_fingerprint(path: &Path) -> Option<FileFingerprint> {
    let metadata = fs::metadata(path).ok()?;

    Some(FileFingerprint {
        length: metadata.len(),
        modified: metadata.modified().ok()?,
    })
}

fn read_session_card_metadata(path: &Path) -> SessionCardMetadata {
    let Ok(file) = File::open(path) else {
        return SessionCardMetadata::default();
    };

    let mut metadata = SessionCardMetadata::default();

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };

        if metadata.working_directory.is_none() {
            metadata.working_directory = extract_working_directory(&value);
        }

        if let Some(text) = extract_user_input_text(&value).and_then(|text| format_preview(&text)) {
            if metadata.first_user_input.is_none() {
                metadata.first_user_input = Some(text.clone());
            }

            metadata.last_user_input = Some(text);
        }

        for message in extract_chat_messages(&value) {
            let Some(preview) = format_preview(&message.text) else {
                continue;
            };

            metadata.last_message_preview = Some(preview);
            metadata.last_message_role = Some(message.role);
        }
    }

    metadata
}

fn extract_working_directory(value: &Value) -> Option<String> {
    let object = value.as_object()?;

    for key in [
        "cwd",
        "working_directory",
        "workingDirectory",
        "workspace",
        "workspace_path",
        "workspacePath",
        "project_path",
        "projectPath",
        "repo_path",
        "repoPath",
        "repository_path",
        "repositoryPath",
    ] {
        let Some(candidate) = object.get(key).and_then(Value::as_str) else {
            continue;
        };

        let candidate = candidate.trim();

        if Path::new(candidate).is_absolute() {
            return Some(candidate.to_string());
        }
    }

    for key in [
        "payload", "message", "event", "data", "item", "record", "context", "metadata",
    ] {
        if let Some(child) = object.get(key) {
            if let Some(working_directory) = extract_working_directory(child) {
                return Some(working_directory);
            }
        }
    }

    None
}

fn extract_chat_message_texts(value: &Value) -> Vec<String> {
    extract_chat_messages(value)
        .into_iter()
        .map(|message| message.text)
        .collect()
}

fn extract_chat_messages(value: &Value) -> Vec<ProviderChatMessage> {
    let mut messages = Vec::new();
    collect_chat_messages(value, &mut messages);
    messages
}

fn collect_chat_messages(value: &Value, messages: &mut Vec<ProviderChatMessage>) {
    let Some(object) = value.as_object() else {
        return;
    };

    if let Some(role) = chat_message_role(value) {
        let mut parts = Vec::new();
        collect_known_text_fields(value, &mut parts);

        if !parts.is_empty() {
            messages.push(ProviderChatMessage {
                role: role.to_string(),
                text: parts.join("\n"),
            });
        }

        return;
    }

    for key in ["payload", "message", "event", "data", "item", "record"] {
        if let Some(child) = object.get(key) {
            collect_chat_messages(child, messages);
        }
    }
}

fn extract_user_input_text(value: &Value) -> Option<String> {
    let mut parts = Vec::new();
    collect_user_input_text(value, &mut parts);

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

fn collect_user_input_text(value: &Value, parts: &mut Vec<String>) {
    let Some(object) = value.as_object() else {
        return;
    };

    if is_user_message_object(value) {
        collect_known_text_fields(value, parts);
        return;
    }

    for key in ["payload", "message", "event", "data", "item", "record"] {
        if let Some(child) = object.get(key) {
            collect_user_input_text(child, parts);
        }
    }
}

fn is_user_message_object(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };

    string_field_is(object.get("role"), &["user"])
        || string_field_is(
            object.get("type"),
            &["user", "human", "user_message", "input"],
        )
        || string_field_is(object.get("author"), &["user", "human"])
        || string_field_is(object.get("speaker"), &["user", "human"])
}

fn chat_message_role(value: &Value) -> Option<&'static str> {
    let Some(object) = value.as_object() else {
        return None;
    };

    for key in ["role", "type", "author", "speaker", "sender"] {
        if let Some(role) = normalized_chat_role(object.get(key)) {
            return Some(role);
        }
    }

    None
}

fn normalized_chat_role(value: Option<&Value>) -> Option<&'static str> {
    let actual = value.and_then(Value::as_str)?.to_ascii_lowercase();

    match actual.as_str() {
        "user" | "human" | "user_message" | "input" => Some("user"),
        "assistant" | "ai" | "assistant_message" | "agent_message" => Some("assistant"),
        _ => None,
    }
}

fn string_field_is(value: Option<&Value>, expected: &[&str]) -> bool {
    let Some(actual) = value.and_then(Value::as_str) else {
        return false;
    };

    expected
        .iter()
        .any(|expected_value| actual.eq_ignore_ascii_case(expected_value))
}

fn collect_known_text_fields(value: &Value, parts: &mut Vec<String>) {
    let Some(object) = value.as_object() else {
        return;
    };

    for key in ["content", "text", "input", "prompt"] {
        if let Some(child) = object.get(key) {
            collect_text_content(child, parts);
        }
    }

    if let Some(message) = object.get("message") {
        collect_text_content(message, parts);
    }
}

fn collect_text_content(value: &Value, parts: &mut Vec<String>) {
    match value {
        Value::String(text) => push_text_part(text, parts),
        Value::Array(items) => {
            for item in items {
                collect_text_content(item, parts);
            }
        }
        Value::Object(object) => {
            if is_tool_or_non_text_content(value) {
                return;
            }

            if let Some(text) = object.get("text") {
                collect_text_content(text, parts);
            }

            if let Some(content) = object.get("content") {
                collect_text_content(content, parts);
            }

            if let Some(message) = object.get("message") {
                collect_text_content(message, parts);
            }
        }
        _ => {}
    }
}

fn is_tool_or_non_text_content(value: &Value) -> bool {
    let Some(content_type) = value
        .as_object()
        .and_then(|object| object.get("type"))
        .and_then(Value::as_str)
    else {
        return false;
    };

    let content_type = content_type.to_ascii_lowercase();
    content_type.contains("tool")
        || content_type.contains("image")
        || content_type.contains("audio")
        || content_type.contains("file")
}

fn push_text_part(text: &str, parts: &mut Vec<String>) {
    let trimmed = text.trim();

    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }
}

fn format_preview(text: &str) -> Option<String> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines = normalized
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(PREVIEW_LINE_LIMIT)
        .collect::<Vec<_>>();

    if lines.is_empty() {
        return None;
    }

    Some(truncate_to_char_limit(
        &lines.join("\n"),
        PREVIEW_CHAR_LIMIT,
    ))
}

fn format_search_snippet(text: &str, query_lower: &str) -> Option<String> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");

    if !normalized.to_lowercase().contains(query_lower) {
        return None;
    }

    let lines = normalized
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    if lines.is_empty() {
        return None;
    }

    let Some(match_index) = lines
        .iter()
        .position(|line| line.to_lowercase().contains(query_lower))
    else {
        return format_preview(&normalized);
    };

    let start = match_index.saturating_sub(1);
    let snippet = lines
        .iter()
        .enumerate()
        .skip(start)
        .take(PREVIEW_LINE_LIMIT)
        .map(|(index, line)| {
            if index == match_index {
                centered_line_snippet(line, query_lower)
            } else {
                truncate_to_char_limit(line, SEARCH_LINE_CHAR_LIMIT)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    Some(truncate_to_char_limit(&snippet, PREVIEW_CHAR_LIMIT))
}

fn centered_line_snippet(line: &str, query_lower: &str) -> String {
    let total_chars = line.chars().count();

    if total_chars <= SEARCH_LINE_CHAR_LIMIT {
        return line.to_string();
    }

    let lower_line = line.to_lowercase();
    let Some(match_byte_index) = lower_line.find(query_lower) else {
        return truncate_to_char_limit(line, SEARCH_LINE_CHAR_LIMIT);
    };

    let match_char_index = lower_line[..match_byte_index].chars().count();
    let query_chars = query_lower.chars().count().max(1);
    let context_chars = SEARCH_LINE_CHAR_LIMIT.saturating_sub(query_chars).max(20) / 2;
    let start = match_char_index.saturating_sub(context_chars);
    let end = (match_char_index + query_chars + context_chars).min(total_chars);

    let mut snippet = String::new();

    if start > 0 {
        snippet.push('…');
    }

    snippet.extend(line.chars().skip(start).take(end.saturating_sub(start)));

    if end < total_chars {
        snippet.push('…');
    }

    snippet
}

fn truncate_to_char_limit(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }

    let mut truncated = value
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashSet;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn provider_registry_exposes_builtin_providers_in_order() {
        let ids = registry()
            .registrations()
            .iter()
            .map(|registration| registration.descriptor.id)
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["codex", "claude"]);
    }

    #[test]
    fn provider_registry_ids_are_unique() {
        let mut ids = HashSet::new();

        for registration in registry().registrations() {
            assert!(
                ids.insert(registration.descriptor.id),
                "duplicate provider id: {}",
                registration.descriptor.id
            );
        }
    }

    #[test]
    fn provider_lookup_uses_registered_descriptors() {
        let provider = by_id("codex").expect("codex provider should be registered");

        assert_eq!(provider.display_name(), "Codex");
        assert_eq!(provider.executable(), "codex");
        assert!(by_id("missing").is_none());
    }

    #[test]
    fn provider_capability_labels_are_stable() {
        assert_eq!(
            ProviderCapabilities::LOCAL_SESSION_FILES.labels(),
            vec!["discover", "search", "history", "resume"]
        );
    }

    #[test]
    fn extracts_codex_payload_user_message_preview() {
        let value = json!({
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [
                    {
                        "type": "input_text",
                        "text": "line one\nline two\nline three\nline four"
                    }
                ]
            }
        });

        let preview = extract_user_input_text(&value).and_then(|text| format_preview(&text));

        assert_eq!(preview, Some("line one\nline two\nline three".to_string()));
    }

    #[test]
    fn extracts_codex_user_message_preview() {
        let value = json!({
            "type": "user_message",
            "message": "start this refactor\nkeep it small"
        });

        let preview = extract_user_input_text(&value).and_then(|text| format_preview(&text));

        assert_eq!(
            preview,
            Some("start this refactor\nkeep it small".to_string())
        );
    }

    #[test]
    fn extracts_claude_user_message_preview() {
        let value = json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "fix the install docs"
                    }
                ]
            }
        });

        let preview = extract_user_input_text(&value).and_then(|text| format_preview(&text));

        assert_eq!(preview, Some("fix the install docs".to_string()));
    }

    #[test]
    fn ignores_claude_tool_result_messages() {
        let value = json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "content": "command output should not become the session preview"
                    }
                ]
            }
        });

        let preview = extract_user_input_text(&value).and_then(|text| format_preview(&text));

        assert_eq!(preview, None);
    }

    #[test]
    fn extracts_codex_assistant_message_for_search() {
        let value = json!({
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "assistant",
                "content": [
                    {
                        "type": "output_text",
                        "text": "The migration should stay backward compatible."
                    }
                ]
            }
        });

        let messages = extract_chat_message_texts(&value);
        let snippet = format_search_snippet(&messages[0], "backward compatible");

        assert_eq!(
            messages,
            vec!["The migration should stay backward compatible.".to_string()]
        );
        assert_eq!(
            snippet,
            Some("The migration should stay backward compatible.".to_string())
        );
    }

    #[test]
    fn extracts_claude_assistant_message_for_search() {
        let value = json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [
                    {
                        "type": "text",
                        "text": "Use a small on-demand scan for accurate search."
                    }
                ]
            }
        });

        let messages = extract_chat_message_texts(&value);
        let snippet = format_search_snippet(&messages[0], "on-demand");

        assert_eq!(
            messages,
            vec!["Use a small on-demand scan for accurate search.".to_string()]
        );
        assert_eq!(
            snippet,
            Some("Use a small on-demand scan for accurate search.".to_string())
        );
    }

    #[test]
    fn session_card_preview_includes_latest_assistant_message() {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "sessiondex-preview-test-{}-{unique_suffix}.jsonl",
            std::process::id()
        ));
        let first_line = json!({
            "type": "user_message",
            "message": "start the release checklist"
        });
        let second_line = json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": "The release checklist is ready for review."
            }
        });

        fs::write(&path, format!("{first_line}\n{second_line}\n"))
            .expect("test jsonl file should be writable");

        let previews = session_card_previews(&path);

        let _ = fs::remove_file(path);

        assert_eq!(
            previews,
            (
                Some("start the release checklist".to_string()),
                Some("start the release checklist".to_string()),
                Some("The release checklist is ready for review.".to_string()),
                Some("assistant".to_string()),
            )
        );
    }

    #[test]
    fn session_card_cache_refreshes_changed_files() {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "sessiondex-cache-test-{}-{unique_suffix}.jsonl",
            std::process::id()
        ));
        let first_line = json!({
            "type": "user_message",
            "message": "first cached preview"
        });
        let second_line = json!({
            "type": "user_message",
            "message": "updated cached preview with a different length"
        });

        fs::write(&path, format!("{first_line}\n")).expect("test jsonl file should be writable");
        let first_preview = session_card_previews(&path);

        fs::write(&path, format!("{second_line}\n")).expect("test jsonl file should be writable");
        let updated_preview = session_card_previews(&path);

        let _ = fs::remove_file(path);

        assert_eq!(first_preview.0, Some("first cached preview".to_string()));
        assert_eq!(
            updated_preview.0,
            Some("updated cached preview with a different length".to_string())
        );
    }

    #[test]
    fn ignores_tool_result_text_for_search() {
        let value = json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "content": "this command output should not be a chat search hit"
                    }
                ]
            }
        });

        assert!(extract_chat_message_texts(&value).is_empty());
    }

    #[test]
    fn extracts_working_directory_from_codex_metadata() {
        let value = json!({
            "type": "session_meta",
            "payload": {
                "id": "abc-123",
                "cwd": "/Users/test/Workspace/session-dex"
            }
        });

        assert_eq!(
            extract_working_directory(&value),
            Some("/Users/test/Workspace/session-dex".to_string())
        );
    }

    #[test]
    fn extracts_working_directory_from_claude_record() {
        let value = json!({
            "type": "user",
            "cwd": "/Users/test/Workspace/claude-project",
            "message": {
                "role": "user",
                "content": "hello"
            }
        });

        assert_eq!(
            extract_working_directory(&value),
            Some("/Users/test/Workspace/claude-project".to_string())
        );
    }

    #[test]
    fn ignores_relative_working_directory_candidates() {
        let value = json!({
            "payload": {
                "cwd": "relative/path"
            }
        });

        assert_eq!(extract_working_directory(&value), None);
    }

    #[test]
    fn finds_chat_history_match_in_jsonl_file() {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "sessiondex-search-test-{}-{unique_suffix}.jsonl",
            std::process::id()
        ));
        let first_line = json!({
            "type": "user_message",
            "message": "ordinary setup request"
        });
        let second_line = json!({
            "type": "agent_message",
            "message": "The exact tiny phrase should still find this session."
        });

        fs::write(&path, format!("{first_line}\n{second_line}\n"))
            .expect("test jsonl file should be writable");

        let snippet = chat_history_match(&path, "tiny phrase");

        let _ = fs::remove_file(path);

        assert_eq!(
            snippet,
            Some("The exact tiny phrase should still find this session.".to_string())
        );
    }
}
