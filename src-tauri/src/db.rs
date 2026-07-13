use crate::models::AppSettings;
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct Database {
    path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SessionDiscovery {
    pub repository_path: Option<String>,
    pub branch_name: Option<String>,
    pub discovered_at: i64,
}

impl Database {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn init(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }

        let conn = self.connection()?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS friendly_names (
                provider TEXT NOT NULL,
                session_id TEXT NOT NULL,
                friendly_name TEXT NOT NULL,
                PRIMARY KEY (provider, session_id)
            );

            CREATE TABLE IF NOT EXISTS hidden_sessions (
                provider TEXT NOT NULL,
                session_id TEXT NOT NULL,
                PRIMARY KEY (provider, session_id)
            );

            CREATE TABLE IF NOT EXISTS pinned_sessions (
                provider TEXT NOT NULL,
                session_id TEXT NOT NULL,
                PRIMARY KEY (provider, session_id)
            );

            CREATE TABLE IF NOT EXISTS recent_resumes (
                provider TEXT NOT NULL,
                session_id TEXT NOT NULL,
                resumed_at INTEGER NOT NULL,
                PRIMARY KEY (provider, session_id)
            );

            CREATE TABLE IF NOT EXISTS favorite_projects (
                working_directory TEXT NOT NULL PRIMARY KEY
            );

            CREATE TABLE IF NOT EXISTS session_discoveries (
                provider TEXT NOT NULL,
                session_id TEXT NOT NULL,
                repository_path TEXT,
                branch_name TEXT,
                discovered_at INTEGER NOT NULL,
                PRIMARY KEY (provider, session_id)
            );

            CREATE TABLE IF NOT EXISTS session_collections (
                provider TEXT NOT NULL,
                session_id TEXT NOT NULL,
                collection_name TEXT NOT NULL,
                PRIMARY KEY (provider, session_id)
            );

            CREATE TABLE IF NOT EXISTS collection_colors (
                collection_name TEXT NOT NULL PRIMARY KEY,
                color_name TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS session_notes (
                provider TEXT NOT NULL,
                session_id TEXT NOT NULL,
                note_text TEXT NOT NULL,
                PRIMARY KEY (provider, session_id)
            );

            CREATE TABLE IF NOT EXISTS session_tags (
                provider TEXT NOT NULL,
                session_id TEXT NOT NULL,
                tag_name TEXT NOT NULL,
                PRIMARY KEY (provider, session_id, tag_name)
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT NOT NULL PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )
        .map_err(|err| err.to_string())?;

        Ok(())
    }

    pub fn friendly_names(&self) -> Result<HashMap<(String, String), String>, String> {
        let conn = self.connection()?;
        let mut statement = conn
            .prepare("SELECT provider, session_id, friendly_name FROM friendly_names")
            .map_err(|err| err.to_string())?;

        let rows = statement
            .query_map([], |row| {
                Ok((
                    (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|err| err.to_string())?;

        let mut values = HashMap::new();
        for row in rows {
            let (key, value) = row.map_err(|err| err.to_string())?;
            values.insert(key, value);
        }

        Ok(values)
    }

    pub fn hidden_sessions(&self) -> Result<HashSet<(String, String)>, String> {
        let conn = self.connection()?;
        let mut statement = conn
            .prepare("SELECT provider, session_id FROM hidden_sessions")
            .map_err(|err| err.to_string())?;

        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|err| err.to_string())?;

        let mut values = HashSet::new();
        for row in rows {
            values.insert(row.map_err(|err| err.to_string())?);
        }

        Ok(values)
    }

    pub fn pinned_sessions(&self) -> Result<HashSet<(String, String)>, String> {
        let conn = self.connection()?;
        let mut statement = conn
            .prepare("SELECT provider, session_id FROM pinned_sessions")
            .map_err(|err| err.to_string())?;

        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|err| err.to_string())?;

        let mut values = HashSet::new();
        for row in rows {
            values.insert(row.map_err(|err| err.to_string())?);
        }

        Ok(values)
    }

    pub fn recent_resumes(&self) -> Result<HashMap<(String, String), i64>, String> {
        let conn = self.connection()?;
        let mut statement = conn
            .prepare("SELECT provider, session_id, resumed_at FROM recent_resumes")
            .map_err(|err| err.to_string())?;

        let rows = statement
            .query_map([], |row| {
                Ok((
                    (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(|err| err.to_string())?;

        let mut values = HashMap::new();
        for row in rows {
            let (key, value) = row.map_err(|err| err.to_string())?;
            values.insert(key, value);
        }

        Ok(values)
    }

    pub fn favorite_projects(&self) -> Result<HashSet<String>, String> {
        let conn = self.connection()?;
        let mut statement = conn
            .prepare("SELECT working_directory FROM favorite_projects")
            .map_err(|err| err.to_string())?;

        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|err| err.to_string())?;

        let mut values = HashSet::new();
        for row in rows {
            values.insert(row.map_err(|err| err.to_string())?);
        }

        Ok(values)
    }

    pub fn session_discoveries(
        &self,
    ) -> Result<HashMap<(String, String), SessionDiscovery>, String> {
        let conn = self.connection()?;
        let mut statement = conn
            .prepare(
                r#"
                SELECT provider, session_id, repository_path, branch_name, discovered_at
                FROM session_discoveries
                "#,
            )
            .map_err(|err| err.to_string())?;

        let rows = statement
            .query_map([], |row| {
                Ok((
                    (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                    SessionDiscovery {
                        repository_path: row.get::<_, Option<String>>(2)?,
                        branch_name: row.get::<_, Option<String>>(3)?,
                        discovered_at: row.get::<_, i64>(4)?,
                    },
                ))
            })
            .map_err(|err| err.to_string())?;

        let mut values = HashMap::new();
        for row in rows {
            let (key, discovery) = row.map_err(|err| err.to_string())?;
            values.insert(key, discovery);
        }

        Ok(values)
    }

    pub fn session_collections(&self) -> Result<HashMap<(String, String), String>, String> {
        let conn = self.connection()?;
        let mut statement = conn
            .prepare("SELECT provider, session_id, collection_name FROM session_collections")
            .map_err(|err| err.to_string())?;

        let rows = statement
            .query_map([], |row| {
                Ok((
                    (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|err| err.to_string())?;

        let mut values = HashMap::new();
        for row in rows {
            let (key, value) = row.map_err(|err| err.to_string())?;
            values.insert(key, value);
        }

        Ok(values)
    }

    pub fn collection_colors(&self) -> Result<HashMap<String, String>, String> {
        let conn = self.connection()?;
        let mut statement = conn
            .prepare("SELECT collection_name, color_name FROM collection_colors")
            .map_err(|err| err.to_string())?;

        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|err| err.to_string())?;

        let mut values = HashMap::new();
        for row in rows {
            let (collection_name, color_name) = row.map_err(|err| err.to_string())?;
            values.insert(collection_name, color_name);
        }

        Ok(values)
    }

    pub fn session_notes(&self) -> Result<HashMap<(String, String), String>, String> {
        let conn = self.connection()?;
        let mut statement = conn
            .prepare("SELECT provider, session_id, note_text FROM session_notes")
            .map_err(|err| err.to_string())?;

        let rows = statement
            .query_map([], |row| {
                Ok((
                    (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|err| err.to_string())?;

        let mut values = HashMap::new();
        for row in rows {
            let (key, value) = row.map_err(|err| err.to_string())?;
            values.insert(key, value);
        }

        Ok(values)
    }

    pub fn session_tags(&self) -> Result<HashMap<(String, String), Vec<String>>, String> {
        let conn = self.connection()?;
        let mut statement = conn
            .prepare(
                "SELECT provider, session_id, tag_name FROM session_tags ORDER BY tag_name ASC",
            )
            .map_err(|err| err.to_string())?;

        let rows = statement
            .query_map([], |row| {
                Ok((
                    (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|err| err.to_string())?;

        let mut values: HashMap<(String, String), Vec<String>> = HashMap::new();
        for row in rows {
            let (key, tag_name) = row.map_err(|err| err.to_string())?;
            values.entry(key).or_default().push(tag_name);
        }

        Ok(values)
    }

    pub fn set_friendly_name(
        &self,
        provider: &str,
        session_id: &str,
        friendly_name: &str,
    ) -> Result<(), String> {
        let conn = self.connection()?;
        let trimmed = friendly_name.trim();

        if trimmed.is_empty() {
            conn.execute(
                "DELETE FROM friendly_names WHERE provider = ?1 AND session_id = ?2",
                params![provider, session_id],
            )
            .map_err(|err| err.to_string())?;
        } else {
            conn.execute(
                r#"
                INSERT INTO friendly_names (provider, session_id, friendly_name)
                VALUES (?1, ?2, ?3)
                ON CONFLICT(provider, session_id)
                DO UPDATE SET friendly_name = excluded.friendly_name
                "#,
                params![provider, session_id, trimmed],
            )
            .map_err(|err| err.to_string())?;
        }

        Ok(())
    }

    pub fn set_session_pinned(
        &self,
        provider: &str,
        session_id: &str,
        is_pinned: bool,
    ) -> Result<(), String> {
        let conn = self.connection()?;

        if is_pinned {
            conn.execute(
                r#"
                INSERT OR IGNORE INTO pinned_sessions (provider, session_id)
                VALUES (?1, ?2)
                "#,
                params![provider, session_id],
            )
            .map_err(|err| err.to_string())?;
        } else {
            conn.execute(
                "DELETE FROM pinned_sessions WHERE provider = ?1 AND session_id = ?2",
                params![provider, session_id],
            )
            .map_err(|err| err.to_string())?;
        }

        Ok(())
    }

    pub fn record_session_resumed(&self, provider: &str, session_id: &str) -> Result<i64, String> {
        let conn = self.connection()?;
        let resumed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| err.to_string())?
            .as_secs() as i64;

        conn.execute(
            r#"
            INSERT INTO recent_resumes (provider, session_id, resumed_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(provider, session_id)
            DO UPDATE SET resumed_at = excluded.resumed_at
            "#,
            params![provider, session_id, resumed_at],
        )
        .map_err(|err| err.to_string())?;

        Ok(resumed_at)
    }

    pub fn set_project_favorite(
        &self,
        working_directory: &str,
        is_favorite: bool,
    ) -> Result<(), String> {
        let conn = self.connection()?;
        let trimmed = working_directory.trim();

        if trimmed.is_empty() {
            return Err("Working directory is required.".to_string());
        }

        if is_favorite {
            conn.execute(
                r#"
                INSERT OR IGNORE INTO favorite_projects (working_directory)
                VALUES (?1)
                "#,
                params![trimmed],
            )
            .map_err(|err| err.to_string())?;
        } else {
            conn.execute(
                "DELETE FROM favorite_projects WHERE working_directory = ?1",
                params![trimmed],
            )
            .map_err(|err| err.to_string())?;
        }

        Ok(())
    }

    pub fn record_session_discovered(
        &self,
        provider: &str,
        session_id: &str,
        repository_path: Option<&str>,
        branch_name: Option<&str>,
    ) -> Result<SessionDiscovery, String> {
        let conn = self.connection()?;
        let repository_path = trimmed_optional(repository_path);
        let branch_name = trimmed_optional(branch_name);
        let discovered_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| err.to_string())?
            .as_secs() as i64;

        conn.execute(
            r#"
            INSERT OR IGNORE INTO session_discoveries (
                provider,
                session_id,
                repository_path,
                branch_name,
                discovered_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![
                provider,
                session_id,
                repository_path.as_deref(),
                branch_name.as_deref(),
                discovered_at
            ],
        )
        .map_err(|err| err.to_string())?;

        Ok(SessionDiscovery {
            repository_path,
            branch_name,
            discovered_at,
        })
    }

    pub fn set_session_discovered_branch(
        &self,
        provider: &str,
        session_id: &str,
        branch_name: &str,
    ) -> Result<(), String> {
        let conn = self.connection()?;
        let branch_name = trimmed_optional(Some(branch_name));
        let discovered_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| err.to_string())?
            .as_secs() as i64;

        conn.execute(
            r#"
            INSERT INTO session_discoveries (
                provider,
                session_id,
                branch_name,
                discovered_at
            )
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(provider, session_id)
            DO UPDATE SET branch_name = excluded.branch_name
            "#,
            params![provider, session_id, branch_name.as_deref(), discovered_at],
        )
        .map_err(|err| err.to_string())?;

        Ok(())
    }

    pub fn set_session_collection(
        &self,
        provider: &str,
        session_id: &str,
        collection_name: &str,
    ) -> Result<(), String> {
        let conn = self.connection()?;
        let trimmed = collection_name.trim();

        if trimmed.is_empty() {
            conn.execute(
                "DELETE FROM session_collections WHERE provider = ?1 AND session_id = ?2",
                params![provider, session_id],
            )
            .map_err(|err| err.to_string())?;
        } else {
            conn.execute(
                r#"
                INSERT INTO session_collections (provider, session_id, collection_name)
                VALUES (?1, ?2, ?3)
                ON CONFLICT(provider, session_id)
                DO UPDATE SET collection_name = excluded.collection_name
                "#,
                params![provider, session_id, trimmed],
            )
            .map_err(|err| err.to_string())?;
        }

        Ok(())
    }

    pub fn set_collection_color(
        &self,
        collection_name: &str,
        color_name: &str,
    ) -> Result<(), String> {
        let conn = self.connection()?;
        let trimmed_collection = collection_name.trim();
        let trimmed_color = color_name.trim();

        if trimmed_collection.is_empty() {
            return Err("Collection name is required.".to_string());
        }

        if trimmed_color.is_empty() || trimmed_color == "none" {
            conn.execute(
                "DELETE FROM collection_colors WHERE collection_name = ?1",
                params![trimmed_collection],
            )
            .map_err(|err| err.to_string())?;
        } else {
            conn.execute(
                r#"
                INSERT INTO collection_colors (collection_name, color_name)
                VALUES (?1, ?2)
                ON CONFLICT(collection_name)
                DO UPDATE SET color_name = excluded.color_name
                "#,
                params![trimmed_collection, trimmed_color],
            )
            .map_err(|err| err.to_string())?;
        }

        Ok(())
    }

    pub fn set_session_note(
        &self,
        provider: &str,
        session_id: &str,
        note_text: &str,
    ) -> Result<(), String> {
        let conn = self.connection()?;
        let trimmed = note_text.trim();

        if trimmed.is_empty() {
            conn.execute(
                "DELETE FROM session_notes WHERE provider = ?1 AND session_id = ?2",
                params![provider, session_id],
            )
            .map_err(|err| err.to_string())?;
        } else {
            conn.execute(
                r#"
                INSERT INTO session_notes (provider, session_id, note_text)
                VALUES (?1, ?2, ?3)
                ON CONFLICT(provider, session_id)
                DO UPDATE SET note_text = excluded.note_text
                "#,
                params![provider, session_id, trimmed],
            )
            .map_err(|err| err.to_string())?;
        }

        Ok(())
    }

    pub fn set_session_tags(
        &self,
        provider: &str,
        session_id: &str,
        tags: &[String],
    ) -> Result<(), String> {
        let mut conn = self.connection()?;
        let transaction = conn.transaction().map_err(|err| err.to_string())?;

        transaction
            .execute(
                "DELETE FROM session_tags WHERE provider = ?1 AND session_id = ?2",
                params![provider, session_id],
            )
            .map_err(|err| err.to_string())?;

        let mut seen_tags = HashSet::new();

        for tag in tags {
            let normalized_tag = normalize_tag_name(tag);

            if normalized_tag.is_empty() || !seen_tags.insert(normalized_tag.clone()) {
                continue;
            }

            transaction
                .execute(
                    r#"
                    INSERT INTO session_tags (provider, session_id, tag_name)
                    VALUES (?1, ?2, ?3)
                    "#,
                    params![provider, session_id, normalized_tag],
                )
                .map_err(|err| err.to_string())?;
        }

        transaction.commit().map_err(|err| err.to_string())?;

        Ok(())
    }

    pub fn hide_session(&self, provider: &str, session_id: &str) -> Result<(), String> {
        let conn = self.connection()?;
        conn.execute(
            r#"
            INSERT OR IGNORE INTO hidden_sessions (provider, session_id)
            VALUES (?1, ?2)
            "#,
            params![provider, session_id],
        )
        .map_err(|err| err.to_string())?;

        Ok(())
    }

    pub fn unhide_session(&self, provider: &str, session_id: &str) -> Result<(), String> {
        let conn = self.connection()?;
        conn.execute(
            "DELETE FROM hidden_sessions WHERE provider = ?1 AND session_id = ?2",
            params![provider, session_id],
        )
        .map_err(|err| err.to_string())?;

        Ok(())
    }

    pub fn get_settings(&self) -> Result<AppSettings, String> {
        let conn = self.connection()?;
        let mut settings = AppSettings::default();

        let mut statement = conn
            .prepare("SELECT key, value FROM settings")
            .map_err(|err| err.to_string())?;

        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|err| err.to_string())?;

        for row in rows {
            let (key, value) = row.map_err(|err| err.to_string())?;

            match key.as_str() {
                "theme" if value == "light" || value == "dark" => settings.theme = value,
                "terminal_executable" if !value.trim().is_empty() => {
                    settings.terminal_executable = Some(value)
                }
                "provider_filter" if !value.trim().is_empty() => settings.provider_filter = value,
                "show_hidden_sessions" => settings.show_hidden_sessions = value == "true",
                _ => {}
            }
        }

        Ok(settings)
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), String> {
        let conn = self.connection()?;
        let theme = if settings.theme == "light" {
            "light"
        } else {
            "dark"
        };

        conn.execute(
            r#"
            INSERT INTO settings (key, value)
            VALUES ('theme', ?1)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
            params![theme],
        )
        .map_err(|err| err.to_string())?;

        match settings.terminal_executable.as_deref().map(str::trim) {
            Some(value) if !value.is_empty() => {
                conn.execute(
                    r#"
                    INSERT INTO settings (key, value)
                    VALUES ('terminal_executable', ?1)
                    ON CONFLICT(key) DO UPDATE SET value = excluded.value
                    "#,
                    params![value],
                )
                .map_err(|err| err.to_string())?;
            }
            _ => {
                conn.execute("DELETE FROM settings WHERE key = 'terminal_executable'", [])
                    .map_err(|err| err.to_string())?;
            }
        }

        let provider_filter = settings.provider_filter.trim();
        let provider_filter = if provider_filter.is_empty() {
            "all"
        } else {
            provider_filter
        };

        conn.execute(
            r#"
            INSERT INTO settings (key, value)
            VALUES ('provider_filter', ?1)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
            params![provider_filter],
        )
        .map_err(|err| err.to_string())?;

        conn.execute(
            r#"
            INSERT INTO settings (key, value)
            VALUES ('show_hidden_sessions', ?1)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
            params![if settings.show_hidden_sessions {
                "true"
            } else {
                "false"
            }],
        )
        .map_err(|err| err.to_string())?;

        Ok(())
    }

    pub fn reset_local_data(&self) -> Result<(), String> {
        let conn = self.connection()?;
        conn.execute_batch(
            r#"
            DELETE FROM friendly_names;
            DELETE FROM hidden_sessions;
            DELETE FROM pinned_sessions;
            DELETE FROM recent_resumes;
            DELETE FROM favorite_projects;
            DELETE FROM session_discoveries;
            DELETE FROM session_collections;
            DELETE FROM collection_colors;
            DELETE FROM session_notes;
            DELETE FROM session_tags;
            DELETE FROM settings;
            "#,
        )
        .map_err(|err| err.to_string())?;

        Ok(())
    }

    fn connection(&self) -> Result<Connection, String> {
        Connection::open(&self.path).map_err(|err| err.to_string())
    }
}

fn normalize_tag_name(tag_name: &str) -> String {
    tag_name.trim().trim_start_matches('#').to_lowercase()
}

fn trimmed_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
