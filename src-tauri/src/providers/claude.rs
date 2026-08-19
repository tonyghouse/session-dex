use super::{
    chat_history_match, collect_jsonl_files, collect_session_directories, home_dir,
    read_session_history, session_sources_in_directory, ProviderCapabilities, ProviderDescriptor,
    ProviderDirectoryInventory, ProviderRegistration, ProviderSessionDirectory,
    ProviderSessionHistory, ProviderSessionSearchMatch, ProviderSessionSource, ResumeCommand,
    SessionProvider,
};
use std::path::{Path, PathBuf};

pub struct ClaudeProvider;

pub const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor {
    id: "claude",
    display_name: "Claude",
    executable: "claude",
    session_store: "~/.claude/projects",
    capabilities: ProviderCapabilities::LOCAL_SESSION_FILES,
};

pub const REGISTRATION: ProviderRegistration = ProviderRegistration {
    descriptor: &DESCRIPTOR,
    factory: create_provider,
};

fn create_provider() -> Box<dyn SessionProvider> {
    Box::new(ClaudeProvider)
}

impl SessionProvider for ClaudeProvider {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn sessions_root(&self) -> Option<PathBuf> {
        Some(home_dir()?.join(".claude").join("projects"))
    }

    fn search_sessions(&self, query: &str) -> Result<Vec<ProviderSessionSearchMatch>, String> {
        let Some(root) = self.sessions_root() else {
            return Ok(Vec::new());
        };

        if !root.exists() {
            return Ok(Vec::new());
        }

        Ok(collect_jsonl_files(&root, 3)
            .into_iter()
            .filter_map(|path| {
                let snippet = chat_history_match(&path, query)?;

                Some(ProviderSessionSearchMatch {
                    session_id: path.file_stem()?.to_string_lossy().to_string(),
                    snippet,
                })
            })
            .collect())
    }

    fn session_history(&self, session_id: &str) -> Result<ProviderSessionHistory, String> {
        let path = self
            .session_path(session_id)
            .ok_or_else(|| format!("Claude session not found: {session_id}"))?;

        read_session_history(&path)
    }

    fn resume_command(&self, session_id: &str, working_directory: Option<&str>) -> ResumeCommand {
        ResumeCommand {
            program: "claude",
            args: vec!["--resume".to_string(), session_id.to_string()],
            working_directory: working_directory.map(str::to_string),
        }
    }

    fn session_directories(
        &self,
        known_directories: &[ProviderSessionDirectory],
    ) -> ProviderDirectoryInventory {
        let Some(root) = self.sessions_root() else {
            return ProviderDirectoryInventory {
                directories: Vec::new(),
                complete: true,
            };
        };

        collect_session_directories(&[(root, true)], 3, known_directories)
    }

    fn sessions_in_directory(
        &self,
        directory: &ProviderSessionDirectory,
    ) -> Result<Vec<ProviderSessionSource>, String> {
        session_sources_in_directory(directory, |path| {
            let session_id = path.file_stem()?.to_string_lossy().to_string();
            let project = path
                .parent()
                .and_then(|parent| parent.file_name())
                .map(|name| name.to_string_lossy().to_string());
            let title = project.map(|project| format!("Claude session in {project}"));
            Some((session_id, title))
        })
    }
}

impl ClaudeProvider {
    fn session_path(&self, session_id: &str) -> Option<PathBuf> {
        let root = self.sessions_root()?;

        if !root.exists() {
            return None;
        }

        collect_jsonl_files(&root, 3)
            .into_iter()
            .find(|path| path_matches_session_id(path, session_id))
    }
}

fn path_matches_session_id(path: &Path, session_id: &str) -> bool {
    path.file_stem()
        .is_some_and(|stem| stem.to_string_lossy() == session_id)
}
