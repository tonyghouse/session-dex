use crate::models::AppSettings;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

#[derive(Debug, Clone)]
pub struct IndexedSession {
    pub provider: String,
    pub session_id: String,
    pub title: Option<String>,
    pub source_directory: String,
    pub source_path: String,
    pub source_size: i64,
    pub source_modified_ns: i64,
    pub last_modified: Option<i64>,
    pub working_directory: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct IndexedSessionDirectory {
    pub source_directory: String,
    pub source_modified_ns: i64,
    pub is_active: bool,
    pub last_scanned_at: i64,
    pub has_sessions: bool,
}

#[derive(Debug, Clone)]
pub struct IndexedDirectoryScan {
    pub directory: IndexedSessionDirectory,
    pub sessions: Vec<IndexedSession>,
}

#[derive(Debug, Clone, Default)]
pub struct SessionMetadataSnapshot {
    pub friendly_names: HashMap<(String, String), String>,
    pub hidden_sessions: HashSet<(String, String)>,
    pub pinned_sessions: HashSet<(String, String)>,
    pub recent_resumes: HashMap<(String, String), i64>,
    pub favorite_projects: HashSet<String>,
    pub session_discoveries: HashMap<(String, String), SessionDiscovery>,
    pub session_collections: HashMap<(String, String), String>,
    pub collection_colors: HashMap<String, String>,
    pub session_notes: HashMap<(String, String), String>,
    pub session_tags: HashMap<(String, String), Vec<String>>,
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
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            "#,
        )
        .map_err(|err| err.to_string())?;
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

            CREATE TABLE IF NOT EXISTS session_index (
                provider TEXT NOT NULL,
                session_id TEXT NOT NULL,
                title TEXT,
                source_directory TEXT NOT NULL,
                source_path TEXT NOT NULL,
                source_size INTEGER NOT NULL,
                source_modified_ns INTEGER NOT NULL,
                last_modified INTEGER,
                working_directory TEXT,
                is_active INTEGER NOT NULL DEFAULT 1,
                PRIMARY KEY (provider, session_id)
            );

            CREATE UNIQUE INDEX IF NOT EXISTS session_index_source_path
            ON session_index (provider, source_path);

            CREATE INDEX IF NOT EXISTS session_index_recent
            ON session_index (is_active, last_modified DESC, provider, session_id);

            CREATE INDEX IF NOT EXISTS session_index_directory
            ON session_index (provider, source_directory);

            CREATE TABLE IF NOT EXISTS session_index_directories (
                provider TEXT NOT NULL,
                source_directory TEXT NOT NULL,
                source_modified_ns INTEGER NOT NULL,
                is_active INTEGER NOT NULL DEFAULT 1,
                last_scanned_at INTEGER NOT NULL,
                PRIMARY KEY (provider, source_directory)
            );

            CREATE TABLE IF NOT EXISTS provider_scan_state (
                provider TEXT NOT NULL PRIMARY KEY,
                last_successful_scan INTEGER NOT NULL
            );
            "#,
        )
        .map_err(|err| err.to_string())?;

        Ok(())
    }

    pub fn indexed_session_directories(
        &self,
        provider: &str,
    ) -> Result<HashMap<String, IndexedSessionDirectory>, String> {
        let conn = self.connection()?;
        let mut statement = conn
            .prepare(
                r#"
                SELECT
                    source_directory,
                    source_modified_ns,
                    is_active,
                    last_scanned_at,
                    EXISTS (
                        SELECT 1
                        FROM session_index
                        WHERE session_index.provider = session_index_directories.provider
                          AND session_index.source_directory = session_index_directories.source_directory
                    )
                FROM session_index_directories
                WHERE provider = ?1
                "#,
            )
            .map_err(|err| err.to_string())?;
        let rows = statement
            .query_map(params![provider], |row| {
                Ok(IndexedSessionDirectory {
                    source_directory: row.get(0)?,
                    source_modified_ns: row.get(1)?,
                    is_active: row.get::<_, i64>(2)? != 0,
                    last_scanned_at: row.get(3)?,
                    has_sessions: row.get::<_, i64>(4)? != 0,
                })
            })
            .map_err(|err| err.to_string())?;
        let mut directories = HashMap::new();

        for row in rows {
            let directory = row.map_err(|err| err.to_string())?;
            directories.insert(directory.source_directory.clone(), directory);
        }

        Ok(directories)
    }

    pub fn provider_last_successful_scan(&self, provider: &str) -> Result<Option<i64>, String> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT last_successful_scan FROM provider_scan_state WHERE provider = ?1",
            params![provider],
            |row| row.get(0),
        )
        .optional()
        .map_err(|err| err.to_string())
    }

    pub fn reconcile_indexed_directories(
        &self,
        provider: &str,
        scans: &[IndexedDirectoryScan],
        removed_directories: &[String],
        successful_scan_at: Option<i64>,
        cleanup_unindexed_metadata: bool,
    ) -> Result<HashSet<String>, String> {
        let mut conn = self.connection()?;
        let transaction = conn.transaction().map_err(|err| err.to_string())?;
        let mut existing_sessions = HashMap::new();

        {
            let mut statement = transaction
                .prepare(
                    r#"
                    SELECT
                        provider,
                        session_id,
                        title,
                        source_directory,
                        source_path,
                        source_size,
                        source_modified_ns,
                        last_modified,
                        working_directory,
                        is_active
                    FROM session_index
                    WHERE provider = ?1 AND source_directory = ?2
                    "#,
                )
                .map_err(|err| err.to_string())?;

            for source_directory in scans
                .iter()
                .map(|scan| scan.directory.source_directory.as_str())
                .chain(removed_directories.iter().map(String::as_str))
            {
                let rows = statement
                    .query_map(
                        params![provider, source_directory],
                        indexed_session_from_row,
                    )
                    .map_err(|err| err.to_string())?;

                for row in rows {
                    let session = row.map_err(|err| err.to_string())?;
                    existing_sessions.insert(session.session_id.clone(), session);
                }
            }
        }

        let mut missing_session_ids = existing_sessions.keys().cloned().collect::<HashSet<_>>();
        let mut changed_session_ids = HashSet::new();

        for scan in scans {
            for session in &scan.sessions {
                missing_session_ids.remove(&session.session_id);

                if existing_sessions
                    .get(&session.session_id)
                    .is_none_or(|existing| indexed_source_changed(existing, session))
                {
                    changed_session_ids.insert(session.session_id.clone());
                }

                transaction
                    .execute(
                        r#"
                        INSERT INTO session_index (
                            provider,
                            session_id,
                            title,
                            source_directory,
                            source_path,
                            source_size,
                            source_modified_ns,
                            last_modified,
                            working_directory,
                            is_active
                        )
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                        ON CONFLICT(provider, session_id) DO UPDATE SET
                            title = excluded.title,
                            source_directory = excluded.source_directory,
                            source_path = excluded.source_path,
                            source_size = excluded.source_size,
                            source_modified_ns = excluded.source_modified_ns,
                            last_modified = excluded.last_modified,
                            working_directory = session_index.working_directory,
                            is_active = excluded.is_active
                        WHERE session_index.title IS NOT excluded.title
                           OR session_index.source_directory != excluded.source_directory
                           OR session_index.source_path != excluded.source_path
                           OR session_index.source_size != excluded.source_size
                           OR session_index.source_modified_ns != excluded.source_modified_ns
                           OR session_index.last_modified IS NOT excluded.last_modified
                           OR session_index.is_active != excluded.is_active
                        "#,
                        params![
                            provider,
                            session.session_id,
                            session.title,
                            session.source_directory,
                            session.source_path,
                            session.source_size,
                            session.source_modified_ns,
                            session.last_modified,
                            session.working_directory,
                            if session.is_active { 1 } else { 0 },
                        ],
                    )
                    .map_err(|err| err.to_string())?;
            }

            transaction
                .execute(
                    r#"
                    INSERT INTO session_index_directories (
                        provider,
                        source_directory,
                        source_modified_ns,
                        is_active,
                        last_scanned_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5)
                    ON CONFLICT(provider, source_directory) DO UPDATE SET
                        source_modified_ns = excluded.source_modified_ns,
                        is_active = excluded.is_active,
                        last_scanned_at = excluded.last_scanned_at
                    "#,
                    params![
                        provider,
                        scan.directory.source_directory,
                        scan.directory.source_modified_ns,
                        if scan.directory.is_active { 1 } else { 0 },
                        scan.directory.last_scanned_at,
                    ],
                )
                .map_err(|err| err.to_string())?;
        }

        delete_session_metadata(&transaction, provider, &missing_session_ids)?;
        changed_session_ids.extend(missing_session_ids);

        for source_directory in removed_directories {
            transaction
                .execute(
                    "DELETE FROM session_index_directories WHERE provider = ?1 AND source_directory = ?2",
                    params![provider, source_directory],
                )
                .map_err(|err| err.to_string())?;
        }

        if let Some(successful_scan_at) = successful_scan_at {
            if cleanup_unindexed_metadata {
                delete_unindexed_session_metadata(&transaction, provider)?;
            }
            transaction
                .execute(
                    r#"
                    INSERT INTO provider_scan_state (provider, last_successful_scan)
                    VALUES (?1, ?2)
                    ON CONFLICT(provider) DO UPDATE SET
                        last_successful_scan = excluded.last_successful_scan
                    "#,
                    params![provider, successful_scan_at],
                )
                .map_err(|err| err.to_string())?;
        }

        transaction.commit().map_err(|err| err.to_string())?;
        Ok(changed_session_ids)
    }

    pub fn indexed_active_sessions(&self) -> Result<Vec<IndexedSession>, String> {
        let conn = self.connection()?;
        let mut statement = conn
            .prepare(
                r#"
                SELECT
                    provider,
                    session_id,
                    title,
                    source_directory,
                    source_path,
                    source_size,
                    source_modified_ns,
                    last_modified,
                    working_directory,
                    is_active
                FROM session_index
                WHERE is_active = 1
                ORDER BY last_modified DESC, provider, session_id
                "#,
            )
            .map_err(|err| err.to_string())?;
        let rows = statement
            .query_map([], indexed_session_from_row)
            .map_err(|err| err.to_string())?;
        let mut sessions = Vec::new();

        for row in rows {
            sessions.push(row.map_err(|err| err.to_string())?);
        }

        Ok(sessions)
    }

    pub fn indexed_sessions_for_keys(
        &self,
        keys: &[(String, String)],
    ) -> Result<Vec<IndexedSession>, String> {
        if keys.len() > 512 {
            let requested_keys = keys.iter().cloned().collect::<HashSet<_>>();
            return Ok(self
                .indexed_active_sessions()?
                .into_iter()
                .filter(|session| {
                    requested_keys.contains(&(session.provider.clone(), session.session_id.clone()))
                })
                .collect());
        }

        let conn = self.connection()?;
        let mut statement = conn
            .prepare(
                r#"
                SELECT
                    provider,
                    session_id,
                    title,
                    source_directory,
                    source_path,
                    source_size,
                    source_modified_ns,
                    last_modified,
                    working_directory,
                    is_active
                FROM session_index
                WHERE provider = ?1 AND session_id = ?2 AND is_active = 1
                "#,
            )
            .map_err(|err| err.to_string())?;
        let mut sessions = Vec::new();

        for (provider, session_id) in keys {
            if let Some(session) = statement
                .query_row(params![provider, session_id], indexed_session_from_row)
                .optional()
                .map_err(|err| err.to_string())?
            {
                sessions.push(session);
            }
        }

        Ok(sessions)
    }

    pub fn all_session_metadata(&self) -> Result<SessionMetadataSnapshot, String> {
        Ok(SessionMetadataSnapshot {
            friendly_names: self.friendly_names()?,
            hidden_sessions: self.hidden_sessions()?,
            pinned_sessions: self.pinned_sessions()?,
            recent_resumes: self.recent_resumes()?,
            favorite_projects: self.favorite_projects()?,
            session_discoveries: self.session_discoveries()?,
            session_collections: self.session_collections()?,
            collection_colors: self.collection_colors()?,
            session_notes: self.session_notes()?,
            session_tags: self.session_tags()?,
        })
    }

    pub fn session_metadata_for_keys(
        &self,
        keys: &[(String, String)],
    ) -> Result<SessionMetadataSnapshot, String> {
        let conn = self.connection()?;
        let mut snapshot = SessionMetadataSnapshot {
            favorite_projects: self.favorite_projects()?,
            ..SessionMetadataSnapshot::default()
        };
        let mut metadata_statement = conn
            .prepare(
                r#"
                SELECT
                    (SELECT friendly_name FROM friendly_names WHERE provider = ?1 AND session_id = ?2),
                    EXISTS (SELECT 1 FROM hidden_sessions WHERE provider = ?1 AND session_id = ?2),
                    EXISTS (SELECT 1 FROM pinned_sessions WHERE provider = ?1 AND session_id = ?2),
                    (SELECT resumed_at FROM recent_resumes WHERE provider = ?1 AND session_id = ?2),
                    (SELECT repository_path FROM session_discoveries WHERE provider = ?1 AND session_id = ?2),
                    (SELECT branch_name FROM session_discoveries WHERE provider = ?1 AND session_id = ?2),
                    (SELECT discovered_at FROM session_discoveries WHERE provider = ?1 AND session_id = ?2),
                    (SELECT collection_name FROM session_collections WHERE provider = ?1 AND session_id = ?2),
                    (
                        SELECT collection_colors.color_name
                        FROM session_collections
                        JOIN collection_colors
                          ON collection_colors.collection_name = session_collections.collection_name
                        WHERE session_collections.provider = ?1
                          AND session_collections.session_id = ?2
                    ),
                    (SELECT note_text FROM session_notes WHERE provider = ?1 AND session_id = ?2)
                "#,
            )
            .map_err(|err| err.to_string())?;
        let mut tags_statement = conn
            .prepare(
                r#"
                SELECT tag_name
                FROM session_tags
                WHERE provider = ?1 AND session_id = ?2
                ORDER BY tag_name ASC
                "#,
            )
            .map_err(|err| err.to_string())?;

        for (provider, session_id) in keys {
            let key = (provider.clone(), session_id.clone());
            let (
                friendly_name,
                is_hidden,
                is_pinned,
                last_resumed,
                repository_path,
                branch_name,
                discovered_at,
                collection,
                collection_color,
                note,
            ) = metadata_statement
                .query_row(params![provider, session_id], |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, i64>(1)? != 0,
                        row.get::<_, i64>(2)? != 0,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, Option<String>>(9)?,
                    ))
                })
                .map_err(|err| err.to_string())?;

            if let Some(friendly_name) = friendly_name {
                snapshot.friendly_names.insert(key.clone(), friendly_name);
            }
            if is_hidden {
                snapshot.hidden_sessions.insert(key.clone());
            }
            if is_pinned {
                snapshot.pinned_sessions.insert(key.clone());
            }
            if let Some(last_resumed) = last_resumed {
                snapshot.recent_resumes.insert(key.clone(), last_resumed);
            }
            if let Some(discovered_at) = discovered_at {
                snapshot.session_discoveries.insert(
                    key.clone(),
                    SessionDiscovery {
                        repository_path,
                        branch_name,
                        discovered_at,
                    },
                );
            }
            if let Some(collection) = collection {
                if let Some(collection_color) = collection_color {
                    snapshot
                        .collection_colors
                        .insert(collection.clone(), collection_color);
                }
                snapshot.session_collections.insert(key.clone(), collection);
            }
            if let Some(note) = note {
                snapshot.session_notes.insert(key.clone(), note);
            }

            let rows = tags_statement
                .query_map(params![provider, session_id], |row| row.get::<_, String>(0))
                .map_err(|err| err.to_string())?;
            let tags = rows
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| err.to_string())?;

            if !tags.is_empty() {
                snapshot.session_tags.insert(key, tags);
            }
        }

        Ok(snapshot)
    }

    pub fn set_indexed_working_directory(
        &self,
        provider: &str,
        session_id: &str,
        working_directory: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.connection()?;
        conn.execute(
            r#"
            UPDATE session_index
            SET working_directory = ?3
            WHERE provider = ?1 AND session_id = ?2
            "#,
            params![provider, session_id, working_directory],
        )
        .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn delete_indexed_session(&self, provider: &str, session_id: &str) -> Result<(), String> {
        let mut conn = self.connection()?;
        let transaction = conn.transaction().map_err(|err| err.to_string())?;
        delete_session_metadata(
            &transaction,
            provider,
            &HashSet::from([session_id.to_string()]),
        )?;
        transaction.commit().map_err(|err| err.to_string())
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
                "hard_delete_sessions" => settings.hard_delete_sessions = value == "true",
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

        conn.execute(
            r#"
            INSERT INTO settings (key, value)
            VALUES ('hard_delete_sessions', ?1)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
            params![if settings.hard_delete_sessions {
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
            DELETE FROM session_index;
            DELETE FROM session_index_directories;
            DELETE FROM provider_scan_state;
            DELETE FROM settings;
            "#,
        )
        .map_err(|err| err.to_string())?;

        Ok(())
    }

    fn connection(&self) -> Result<Connection, String> {
        let connection = Connection::open(&self.path).map_err(|err| err.to_string())?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|err| err.to_string())?;
        Ok(connection)
    }
}

fn delete_unindexed_session_metadata(
    transaction: &Transaction<'_>,
    provider: &str,
) -> Result<(), String> {
    for table in [
        "friendly_names",
        "hidden_sessions",
        "pinned_sessions",
        "recent_resumes",
        "session_discoveries",
        "session_collections",
        "session_notes",
        "session_tags",
    ] {
        transaction
            .execute(
                &format!(
                    r#"
                    DELETE FROM {table}
                    WHERE provider = ?1
                      AND NOT EXISTS (
                          SELECT 1
                          FROM session_index
                          WHERE session_index.provider = {table}.provider
                            AND session_index.session_id = {table}.session_id
                      )
                    "#
                ),
                params![provider],
            )
            .map_err(|err| err.to_string())?;
    }

    Ok(())
}

fn indexed_session_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<IndexedSession> {
    Ok(IndexedSession {
        provider: row.get(0)?,
        session_id: row.get(1)?,
        title: row.get(2)?,
        source_directory: row.get(3)?,
        source_path: row.get(4)?,
        source_size: row.get(5)?,
        source_modified_ns: row.get(6)?,
        last_modified: row.get(7)?,
        working_directory: row.get(8)?,
        is_active: row.get::<_, i64>(9)? != 0,
    })
}

fn indexed_source_changed(existing: &IndexedSession, current: &IndexedSession) -> bool {
    existing.title != current.title
        || existing.source_directory != current.source_directory
        || existing.source_path != current.source_path
        || existing.source_size != current.source_size
        || existing.source_modified_ns != current.source_modified_ns
        || existing.last_modified != current.last_modified
        || existing.is_active != current.is_active
}

fn delete_session_metadata(
    transaction: &Transaction<'_>,
    provider: &str,
    session_ids: &HashSet<String>,
) -> Result<(), String> {
    if session_ids.is_empty() {
        return Ok(());
    }

    transaction
        .execute_batch(
            r#"
            CREATE TEMP TABLE IF NOT EXISTS sessiondex_removed_sessions (
                session_id TEXT NOT NULL PRIMARY KEY
            );
            DELETE FROM sessiondex_removed_sessions;
            "#,
        )
        .map_err(|err| err.to_string())?;

    {
        let mut insert = transaction
            .prepare("INSERT OR IGNORE INTO sessiondex_removed_sessions (session_id) VALUES (?1)")
            .map_err(|err| err.to_string())?;

        for session_id in session_ids {
            insert
                .execute(params![session_id])
                .map_err(|err| err.to_string())?;
        }
    }

    for table in [
        "friendly_names",
        "hidden_sessions",
        "pinned_sessions",
        "recent_resumes",
        "session_discoveries",
        "session_collections",
        "session_notes",
        "session_tags",
        "session_index",
    ] {
        transaction
            .execute(
                &format!(
                    "DELETE FROM {table} WHERE provider = ?1 AND session_id IN (SELECT session_id FROM sessiondex_removed_sessions)"
                ),
                params![provider],
            )
            .map_err(|err| err.to_string())?;
    }

    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_database() -> (Database, PathBuf) {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "sessiondex-db-test-{}-{unique_suffix}.sqlite3",
            std::process::id()
        ));
        let database = Database::new(path.clone());
        database.init().expect("test database should initialize");
        (database, path)
    }

    #[test]
    fn reconciliation_removes_dangling_session_metadata() {
        let (database, path) = test_database();
        let provider = "codex";
        let session_id = "11111111-1111-4111-8111-111111111111";
        let source_directory = "/tmp/sessiondex-provider-sessions";
        let directory = IndexedSessionDirectory {
            source_directory: source_directory.to_string(),
            source_modified_ns: 10,
            is_active: true,
            last_scanned_at: 10,
            has_sessions: true,
        };
        let indexed_session = IndexedSession {
            provider: provider.to_string(),
            session_id: session_id.to_string(),
            title: Some("Test session".to_string()),
            source_directory: source_directory.to_string(),
            source_path: format!("{source_directory}/{session_id}.jsonl"),
            source_size: 100,
            source_modified_ns: 10,
            last_modified: Some(10),
            working_directory: Some("/tmp/project".to_string()),
            is_active: true,
        };
        let orphan_session_id = "99999999-9999-4999-8999-999999999999";

        database
            .set_friendly_name(provider, orphan_session_id, "Orphan")
            .expect("legacy orphan metadata should save");

        let initial_changes = database
            .reconcile_indexed_directories(
                provider,
                &[IndexedDirectoryScan {
                    directory: directory.clone(),
                    sessions: vec![indexed_session],
                }],
                &[],
                Some(10),
                true,
            )
            .expect("session should be indexed");
        assert!(initial_changes.contains(session_id));
        assert!(!database
            .friendly_names()
            .unwrap()
            .contains_key(&(provider.to_string(), orphan_session_id.to_string())));
        database
            .set_friendly_name(provider, session_id, "Friendly")
            .expect("friendly name should save");
        database
            .set_session_pinned(provider, session_id, true)
            .expect("pin should save");
        database
            .hide_session(provider, session_id)
            .expect("hidden state should save");
        database
            .set_session_note(provider, session_id, "Remember this")
            .expect("note should save");
        database
            .set_session_tags(provider, session_id, &["cleanup".to_string()])
            .expect("tag should save");
        let key = (provider.to_string(), session_id.to_string());
        let metadata = database
            .session_metadata_for_keys(std::slice::from_ref(&key))
            .expect("targeted metadata should load");
        assert_eq!(
            metadata.friendly_names.get(&key).map(String::as_str),
            Some("Friendly")
        );
        assert!(metadata.pinned_sessions.contains(&key));
        assert!(metadata.hidden_sessions.contains(&key));
        assert_eq!(
            metadata.session_notes.get(&key).map(String::as_str),
            Some("Remember this")
        );
        assert_eq!(metadata.session_tags.get(&key).unwrap(), &["cleanup"]);

        let removal_changes = database
            .reconcile_indexed_directories(
                provider,
                &[IndexedDirectoryScan {
                    directory,
                    sessions: Vec::new(),
                }],
                &[],
                Some(20),
                false,
            )
            .expect("empty authoritative scan should reconcile");
        assert!(removal_changes.contains(session_id));

        assert!(database.indexed_active_sessions().unwrap().is_empty());
        assert!(!database.friendly_names().unwrap().contains_key(&key));
        assert!(!database.pinned_sessions().unwrap().contains(&key));
        assert!(!database.hidden_sessions().unwrap().contains(&key));
        assert!(!database.session_notes().unwrap().contains_key(&key));
        assert!(!database.session_tags().unwrap().contains_key(&key));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn moving_an_indexed_session_preserves_its_metadata() {
        let (database, path) = test_database();
        let provider = "codex";
        let session_id = "22222222-2222-4222-8222-222222222222";
        let active_directory = "/tmp/sessiondex-active";
        let archived_directory = "/tmp/sessiondex-archived";
        let session = |source_directory: &str, is_active: bool| IndexedSession {
            provider: provider.to_string(),
            session_id: session_id.to_string(),
            title: Some("Movable session".to_string()),
            source_directory: source_directory.to_string(),
            source_path: format!("{source_directory}/{session_id}.jsonl"),
            source_size: 100,
            source_modified_ns: 10,
            last_modified: Some(10),
            working_directory: Some("/tmp/project".to_string()),
            is_active,
        };

        database
            .reconcile_indexed_directories(
                provider,
                &[IndexedDirectoryScan {
                    directory: IndexedSessionDirectory {
                        source_directory: active_directory.to_string(),
                        source_modified_ns: 10,
                        is_active: true,
                        last_scanned_at: 10,
                        has_sessions: true,
                    },
                    sessions: vec![session(active_directory, true)],
                }],
                &[],
                Some(10),
                true,
            )
            .unwrap();
        database
            .set_friendly_name(provider, session_id, "Keep me")
            .unwrap();

        database
            .reconcile_indexed_directories(
                provider,
                &[
                    IndexedDirectoryScan {
                        directory: IndexedSessionDirectory {
                            source_directory: active_directory.to_string(),
                            source_modified_ns: 20,
                            is_active: true,
                            last_scanned_at: 20,
                            has_sessions: false,
                        },
                        sessions: Vec::new(),
                    },
                    IndexedDirectoryScan {
                        directory: IndexedSessionDirectory {
                            source_directory: archived_directory.to_string(),
                            source_modified_ns: 20,
                            is_active: false,
                            last_scanned_at: 20,
                            has_sessions: true,
                        },
                        sessions: vec![session(archived_directory, false)],
                    },
                ],
                &[],
                Some(20),
                false,
            )
            .unwrap();

        let key = (provider.to_string(), session_id.to_string());
        assert_eq!(
            database.friendly_names().unwrap().get(&key),
            Some(&"Keep me".to_string())
        );
        assert!(database.indexed_active_sessions().unwrap().is_empty());

        let _ = fs::remove_file(path);
    }
}
