use super::{
    chat_history_match, collect_jsonl_files, home_dir, modified_seconds, read_session_history,
    session_card_previews, session_working_directory, sort_recent_first, ProviderCapabilities,
    ProviderDescriptor, ProviderRegistration, ProviderSession, ProviderSessionHistory,
    ProviderSessionSearchMatch, ResumeCommand, SessionProvider,
};
use std::path::{Path, PathBuf};

pub struct CodexProvider;

pub const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    id: "codex",
    display_name: "Codex",
    executable: "codex",
    session_store: "~/.codex/sessions",
    capabilities: ProviderCapabilities::LOCAL_SESSION_FILES,
};

pub const REGISTRATION: ProviderRegistration = ProviderRegistration {
    descriptor: &DESCRIPTOR,
    factory: create_provider,
};

fn create_provider() -> Box<dyn SessionProvider> {
    Box::new(CodexProvider)
}

impl SessionProvider for CodexProvider {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn sessions_root(&self) -> Option<PathBuf> {
        Some(home_dir()?.join(".codex").join("sessions"))
    }

    fn list_sessions(&self) -> Result<Vec<ProviderSession>, String> {
        let Some(root) = self.sessions_root() else {
            return Ok(Vec::new());
        };

        if !root.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = collect_jsonl_files(&root, 5)
            .into_iter()
            .filter_map(|path| {
                let stem = path.file_stem()?.to_string_lossy().to_string();
                let session_id = extract_codex_session_id(&stem);
                let title = stem.strip_prefix("rollout-").unwrap_or(&stem).to_string();
                let (first_user_input, last_user_input, last_message_preview, last_message_role) =
                    session_card_previews(&path);
                let working_directory = session_working_directory(&path);

                Some(ProviderSession {
                    session_id,
                    title: Some(title),
                    first_user_input,
                    last_user_input,
                    last_message_preview,
                    last_message_role,
                    working_directory,
                    last_modified: modified_seconds(&path),
                })
            })
            .collect::<Vec<_>>();

        sort_recent_first(&mut sessions);
        Ok(sessions)
    }

    fn search_sessions(&self, query: &str) -> Result<Vec<ProviderSessionSearchMatch>, String> {
        let Some(root) = self.sessions_root() else {
            return Ok(Vec::new());
        };

        if !root.exists() {
            return Ok(Vec::new());
        }

        Ok(collect_jsonl_files(&root, 5)
            .into_iter()
            .filter_map(|path| {
                let snippet = chat_history_match(&path, query)?;
                let stem = path.file_stem()?.to_string_lossy().to_string();

                Some(ProviderSessionSearchMatch {
                    session_id: extract_codex_session_id(&stem),
                    snippet,
                })
            })
            .collect())
    }

    fn session_history(&self, session_id: &str) -> Result<ProviderSessionHistory, String> {
        let path = self
            .session_path(session_id)
            .ok_or_else(|| format!("Codex session not found: {session_id}"))?;

        read_session_history(&path)
    }

    fn resume_command(&self, session_id: &str, working_directory: Option<&str>) -> ResumeCommand {
        ResumeCommand {
            program: "codex",
            args: vec!["resume".to_string(), session_id.to_string()],
            working_directory: working_directory.map(str::to_string),
        }
    }
}

impl CodexProvider {
    fn session_path(&self, session_id: &str) -> Option<PathBuf> {
        let root = self.sessions_root()?;

        if !root.exists() {
            return None;
        }

        collect_jsonl_files(&root, 5)
            .into_iter()
            .find(|path| codex_path_matches_session_id(path, session_id))
    }
}

fn codex_path_matches_session_id(path: &Path, session_id: &str) -> bool {
    let Some(stem) = path.file_stem().map(|stem| stem.to_string_lossy()) else {
        return false;
    };

    stem == session_id || extract_codex_session_id(&stem) == session_id
}

fn extract_codex_session_id(stem: &str) -> String {
    if stem.len() >= 36 {
        let candidate = &stem[stem.len() - 36..];
        if looks_like_uuid(candidate) {
            return candidate.to_string();
        }
    }

    stem.strip_prefix("rollout-").unwrap_or(stem).to_string()
}

fn looks_like_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();

    if bytes.len() != 36 {
        return false;
    }

    for (index, byte) in bytes.iter().enumerate() {
        match index {
            8 | 13 | 18 | 23 => {
                if *byte != b'-' {
                    return false;
                }
            }
            _ => {
                if !byte.is_ascii_hexdigit() {
                    return false;
                }
            }
        }
    }

    true
}
