mod db;
mod instance;
mod models;
mod providers;
mod system_profile;
mod terminal;

use db::{
    Database, IndexedDirectoryScan, IndexedSession, IndexedSessionDirectory, SessionDiscovery,
    SessionMetadataSnapshot,
};
use instance::{InstanceClaim, InstanceMessage, InstanceOwner};
use models::{
    AppSettings, DeleteResult, ProviderStatus, SessionCardDetails, SessionCardRequest,
    SessionHistory, SessionIdentity, SessionMessage, SessionRecord, SessionRefresh,
    SessionSearchResult, UninstallResult,
};
use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{Manager, WindowEvent};

const FRIENDLY_NAME_MAX_CHARS: usize = 100;
const COLLECTION_NAME_MAX_CHARS: usize = 48;
const TAG_NAME_MAX_CHARS: usize = 32;
const SESSION_SCAN_BUFFER_SECONDS: i64 = 10 * 60;
const SESSION_DIRECTORY_AUDIT_BATCH: usize = 32;
const TARGETED_SESSION_METADATA_LIMIT: usize = 512;

#[derive(Clone)]
struct AppState {
    db: Database,
}

#[derive(Debug, Clone, Default)]
struct GitSnapshot {
    repository_path: Option<String>,
    branch_name: Option<String>,
}

#[tauri::command]
async fn list_sessions(app: tauri::AppHandle) -> Result<Vec<SessionRecord>, String> {
    let db = app.state::<AppState>().db.clone();

    run_blocking(app, "list_sessions", move || list_sessions_inner(&db)).await
}

#[tauri::command]
async fn refresh_sessions(app: tauri::AppHandle) -> Result<SessionRefresh, String> {
    let db = app.state::<AppState>().db.clone();

    run_blocking(app, "refresh_sessions", move || {
        let changed_sessions = reconcile_session_index(&db)?;
        let changed_keys = changed_sessions.iter().cloned().collect::<Vec<_>>();
        let indexed_sessions = db.indexed_sessions_for_keys(&changed_keys)?;
        let active_keys = indexed_sessions
            .iter()
            .map(|session| (session.provider.clone(), session.session_id.clone()))
            .collect::<HashSet<_>>();
        let upserted = build_session_records(&db, indexed_sessions)?;
        let removed = changed_sessions
            .into_iter()
            .filter(|key| !active_keys.contains(key))
            .map(|(provider, session_id)| SessionIdentity {
                provider,
                session_id,
            })
            .collect();

        Ok(SessionRefresh { upserted, removed })
    })
    .await
}

fn list_sessions_inner(db: &Database) -> Result<Vec<SessionRecord>, String> {
    build_session_records(db, db.indexed_active_sessions()?)
}

fn build_session_records(
    db: &Database,
    indexed_sessions: Vec<IndexedSession>,
) -> Result<Vec<SessionRecord>, String> {
    let metadata = if indexed_sessions.len() <= TARGETED_SESSION_METADATA_LIMIT {
        let keys = indexed_sessions
            .iter()
            .map(|session| (session.provider.clone(), session.session_id.clone()))
            .collect::<Vec<_>>();
        db.session_metadata_for_keys(&keys)?
    } else {
        db.all_session_metadata()?
    };
    let SessionMetadataSnapshot {
        friendly_names,
        hidden_sessions,
        pinned_sessions,
        recent_resumes,
        favorite_projects,
        mut session_discoveries,
        session_collections,
        collection_colors,
        session_notes,
        session_tags,
    } = metadata;
    let mut git_snapshots = HashMap::new();
    let mut records = Vec::new();
    let provider_instances = providers::all()
        .into_iter()
        .map(|provider| (provider.id().to_string(), provider))
        .collect::<HashMap<_, _>>();
    let provider_capabilities = provider_instances
        .iter()
        .map(|(provider_id, provider)| {
            (
                provider_id.clone(),
                (
                    terminal::command_available(provider.executable()),
                    provider.supports_delete(),
                ),
            )
        })
        .collect::<HashMap<_, _>>();

    for session in indexed_sessions {
        let Some(provider) = provider_instances.get(&session.provider) else {
            continue;
        };
        let (provider_available, can_delete) = provider_capabilities
            .get(&session.provider)
            .copied()
            .unwrap_or_default();
        let source_path = Path::new(&session.source_path);
        let cached_card = providers::cached_session_card(
            source_path,
            session.source_size,
            session.source_modified_ns,
        );
        let working_directory = cached_card
            .as_ref()
            .and_then(|card| card.working_directory.clone())
            .or(session.working_directory);
        let key = (session.provider.clone(), session.session_id.clone());
        let is_hidden = hidden_sessions.contains(&key);
        let is_pinned = pinned_sessions.contains(&key);
        let last_resumed = recent_resumes.get(&key).copied();
        let is_favorite_project = working_directory
            .as_deref()
            .map(str::trim)
            .filter(|working_directory| !working_directory.is_empty())
            .is_some_and(|working_directory| favorite_projects.contains(working_directory));
        let discovery = if working_directory.is_some() {
            Some(session_discovery(
                db,
                &mut session_discoveries,
                &mut git_snapshots,
                &key,
                working_directory.as_deref(),
            )?)
        } else {
            session_discoveries.get(&key).cloned()
        };

        let friendly_name = friendly_names.get(&key).cloned();
        let collection = session_collections.get(&key).cloned();
        let collection_color = collection
            .as_deref()
            .and_then(|collection_name| collection_colors.get(collection_name))
            .cloned();
        let note = session_notes.get(&key).cloned();
        let tags = session_tags.get(&key).cloned().unwrap_or_default();
        let display_name = friendly_name
            .clone()
            .or_else(|| session.title.clone())
            .unwrap_or_else(|| session.session_id.clone());
        let resume_command = terminal::shell_command(
            &provider.resume_command(&session.session_id, working_directory.as_deref()),
        );
        let source_version = format!("{}:{}", session.source_size, session.source_modified_ns);

        records.push(SessionRecord {
            provider: session.provider,
            provider_display_name: provider.display_name().to_string(),
            session_id: session.session_id,
            title: session.title,
            friendly_name,
            collection,
            collection_color,
            note,
            tags,
            display_name,
            first_user_input: cached_card
                .as_ref()
                .and_then(|card| card.first_user_input.clone()),
            last_user_input: cached_card
                .as_ref()
                .and_then(|card| card.last_user_input.clone()),
            last_message_preview: cached_card
                .as_ref()
                .and_then(|card| card.last_message_preview.clone()),
            last_message_role: cached_card
                .as_ref()
                .and_then(|card| card.last_message_role.clone()),
            working_directory,
            discovered_repository: discovery
                .as_ref()
                .and_then(|discovery| discovery.repository_path.clone()),
            discovered_branch: discovery
                .as_ref()
                .and_then(|discovery| discovery.branch_name.clone()),
            discovered_at: discovery.map(|discovery| discovery.discovered_at),
            resume_command,
            last_modified: session.last_modified,
            source_version,
            last_resumed,
            can_delete,
            can_resume: provider_available,
            is_hidden,
            is_pinned,
            is_favorite_project,
        });
    }

    records.sort_by(|left, right| right.last_modified.cmp(&left.last_modified));
    Ok(records)
}

fn reconcile_session_index(db: &Database) -> Result<HashSet<(String, String)>, String> {
    let scanned_at = now_seconds()?;
    let mut changed_sessions = HashSet::new();

    for provider in providers::all() {
        let provider_id = provider.id();
        let existing_directories = db.indexed_session_directories(provider_id)?;
        let last_successful_scan = db.provider_last_successful_scan(provider_id)?;
        let had_successful_scan = last_successful_scan.is_some();
        let known_directories = if last_successful_scan.is_some() {
            existing_directories
                .values()
                .map(|directory| providers::ProviderSessionDirectory {
                    path: PathBuf::from(&directory.source_directory),
                    modified_ns: directory.source_modified_ns,
                    is_active: directory.is_active,
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let inventory = provider.session_directories(&known_directories);
        let last_successful_scan = last_successful_scan.unwrap_or(0);
        let buffered_cutoff = last_successful_scan.saturating_sub(SESSION_SCAN_BUFFER_SECONDS);
        let current_directories = inventory
            .directories
            .iter()
            .map(|directory| directory.path.to_string_lossy().to_string())
            .collect::<HashSet<_>>();
        let removed_directories = if inventory.complete {
            existing_directories
                .keys()
                .filter(|directory| !current_directories.contains(*directory))
                .cloned()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let mut selected_directories = Vec::new();
        let mut audit_candidates = Vec::new();

        for directory in inventory.directories {
            let source_directory = directory.path.to_string_lossy().to_string();
            let previous = existing_directories.get(&source_directory);
            let directory_modified_seconds = directory.modified_ns / 1_000_000_000;
            let changed = previous.is_none_or(|previous| {
                previous.source_modified_ns != directory.modified_ns
                    || previous.is_active != directory.is_active
            });
            let inside_buffer = directory_modified_seconds >= buffered_cutoff;

            if changed || inside_buffer {
                selected_directories.push(directory);
            } else if previous.is_some_and(|previous| previous.has_sessions) {
                audit_candidates.push((
                    previous
                        .map(|previous| previous.last_scanned_at)
                        .unwrap_or(0),
                    directory,
                ));
            }
        }

        audit_candidates.sort_by_key(|(last_scanned_at, _)| *last_scanned_at);
        selected_directories.extend(
            audit_candidates
                .into_iter()
                .take(SESSION_DIRECTORY_AUDIT_BATCH)
                .map(|(_, directory)| directory),
        );

        let mut scans = Vec::new();
        let mut selected_scans_complete = true;

        for directory in selected_directories {
            match provider.sessions_in_directory(&directory) {
                Ok(sessions) => {
                    let source_directory = directory.path.to_string_lossy().to_string();
                    scans.push(IndexedDirectoryScan {
                        directory: IndexedSessionDirectory {
                            source_directory: source_directory.clone(),
                            source_modified_ns: directory.modified_ns,
                            is_active: directory.is_active,
                            last_scanned_at: scanned_at,
                            has_sessions: !sessions.is_empty(),
                        },
                        sessions: sessions
                            .into_iter()
                            .map(|session| IndexedSession {
                                provider: provider_id.to_string(),
                                session_id: session.session_id,
                                title: session.title,
                                source_directory: source_directory.clone(),
                                source_path: session.source_path.to_string_lossy().to_string(),
                                source_size: session.source_size,
                                source_modified_ns: session.source_modified_ns,
                                last_modified: session.last_modified,
                                working_directory: None,
                                is_active: session.is_active,
                            })
                            .collect(),
                    });
                }
                Err(_) => selected_scans_complete = false,
            }
        }

        // If the same provider ID appears in active and archived storage,
        // prefer the active source regardless of directory traversal order.
        scans.sort_by_key(|scan| scan.directory.is_active);

        let successful_scan_at =
            (inventory.complete && selected_scans_complete).then_some(scanned_at);
        let provider_changes = db.reconcile_indexed_directories(
            provider_id,
            &scans,
            &removed_directories,
            successful_scan_at,
            !had_successful_scan && successful_scan_at.is_some(),
        )?;

        changed_sessions.extend(
            provider_changes
                .into_iter()
                .map(|session_id| (provider_id.to_string(), session_id)),
        );
    }

    Ok(changed_sessions)
}

fn now_seconds() -> Result<i64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?;
    Ok(i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
}

#[tauri::command]
async fn hydrate_session_cards(
    app: tauri::AppHandle,
    sessions: Vec<SessionCardRequest>,
) -> Result<Vec<SessionCardDetails>, String> {
    if sessions.len() > 60 {
        return Err("Session card hydration is limited to 60 sessions per request.".to_string());
    }

    let db = app.state::<AppState>().db.clone();

    run_blocking(app, "hydrate_session_cards", move || {
        let keys = sessions
            .into_iter()
            .map(|session| (session.provider, session.session_id))
            .collect::<Vec<_>>();

        for (provider, _) in &keys {
            validate_provider(provider)?;
        }

        let indexed_sessions = db.indexed_sessions_for_keys(&keys)?;
        let mut session_discoveries = db.session_discoveries()?;
        let favorite_projects = db.favorite_projects()?;
        let mut git_snapshots = HashMap::new();
        let provider_instances = providers::all()
            .into_iter()
            .map(|provider| (provider.id().to_string(), provider))
            .collect::<HashMap<_, _>>();
        let mut details = Vec::new();

        for session in indexed_sessions {
            let Some(provider) = provider_instances.get(&session.provider) else {
                continue;
            };
            let source_path = Path::new(&session.source_path);

            if !source_path.is_file() {
                continue;
            }

            let card = providers::load_session_card(source_path);
            let working_directory = card
                .working_directory
                .clone()
                .or(session.working_directory.clone());

            if let Some(working_directory) = card.working_directory.as_deref() {
                db.set_indexed_working_directory(
                    &session.provider,
                    &session.session_id,
                    Some(working_directory),
                )?;
            }

            let key = (session.provider.clone(), session.session_id.clone());
            let discovery = if working_directory.is_some() {
                Some(session_discovery(
                    &db,
                    &mut session_discoveries,
                    &mut git_snapshots,
                    &key,
                    working_directory.as_deref(),
                )?)
            } else {
                session_discoveries.get(&key).cloned()
            };
            let is_favorite_project = working_directory
                .as_deref()
                .map(str::trim)
                .filter(|working_directory| !working_directory.is_empty())
                .is_some_and(|working_directory| favorite_projects.contains(working_directory));
            let resume_command = terminal::shell_command(
                &provider.resume_command(&session.session_id, working_directory.as_deref()),
            );

            details.push(SessionCardDetails {
                provider: session.provider,
                session_id: session.session_id,
                source_version: format!("{}:{}", session.source_size, session.source_modified_ns),
                first_user_input: card.first_user_input,
                last_user_input: card.last_user_input,
                last_message_preview: card.last_message_preview,
                last_message_role: card.last_message_role,
                working_directory,
                discovered_repository: discovery
                    .as_ref()
                    .and_then(|discovery| discovery.repository_path.clone()),
                discovered_branch: discovery
                    .as_ref()
                    .and_then(|discovery| discovery.branch_name.clone()),
                discovered_at: discovery.map(|discovery| discovery.discovered_at),
                resume_command,
                is_favorite_project,
            });
        }

        Ok(details)
    })
    .await
}

fn session_discovery(
    db: &Database,
    session_discoveries: &mut HashMap<(String, String), SessionDiscovery>,
    git_snapshots: &mut HashMap<String, GitSnapshot>,
    key: &(String, String),
    working_directory: Option<&str>,
) -> Result<SessionDiscovery, String> {
    if let Some(discovery) = session_discoveries.get(key) {
        return Ok(discovery.clone());
    }

    let git_snapshot = working_directory
        .and_then(normalized_path)
        .map(|working_directory| {
            git_snapshots
                .entry(working_directory.clone())
                .or_insert_with(|| git_snapshot_at_discovery(&working_directory))
                .clone()
        })
        .unwrap_or_default();

    let discovery = db.record_session_discovered(
        &key.0,
        &key.1,
        git_snapshot.repository_path.as_deref(),
        git_snapshot.branch_name.as_deref(),
    )?;

    session_discoveries.insert(key.clone(), discovery.clone());

    Ok(discovery)
}

fn git_snapshot_at_discovery(working_directory: &str) -> GitSnapshot {
    if !Path::new(working_directory).is_dir() {
        return GitSnapshot::default();
    }

    GitSnapshot {
        repository_path: git_output(working_directory, &["rev-parse", "--show-toplevel"]),
        branch_name: git_branch_at_discovery(working_directory),
    }
}

fn git_branch_at_discovery(working_directory: &str) -> Option<String> {
    git_output(working_directory, &["branch", "--show-current"])
        .or_else(|| git_output(working_directory, &["rev-parse", "--abbrev-ref", "HEAD"]))
        .filter(|branch| branch != "HEAD")
}

fn git_output(working_directory: &str, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(working_directory)
        .args(args)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    normalized_path(&String::from_utf8_lossy(&output.stdout))
}

fn git_branch_options(repository_path: &str) -> Vec<String> {
    let Some(repository_path) = normalized_path(repository_path) else {
        return Vec::new();
    };

    if !Path::new(&repository_path).is_dir() {
        return Vec::new();
    }

    let Ok(output) = Command::new("git")
        .arg("-C")
        .arg(&repository_path)
        .args([
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads",
            "refs/remotes",
        ])
        .output()
    else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    let mut seen_branches = HashSet::new();
    let mut branches = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(normalized_path)
        .filter(|branch| !branch.ends_with("/HEAD"))
        .filter(|branch| seen_branches.insert(branch.clone()))
        .collect::<Vec<_>>();

    branches.sort_by_key(|branch| branch.to_lowercase());
    branches
}

fn normalized_path(path: &str) -> Option<String> {
    let trimmed = path.trim();

    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

async fn run_blocking<T, F>(
    app: tauri::AppHandle,
    operation: &'static str,
    task: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    match tauri::async_runtime::spawn_blocking(task).await {
        Ok(result) => result,
        Err(error) => {
            let message = format!("SessionDex {operation} worker failed: {error}");
            eprintln!("{message}");
            app.exit(1);
            Err(message)
        }
    }
}

#[tauri::command]
async fn search_sessions(
    app: tauri::AppHandle,
    query: String,
    provider_filter: Option<String>,
) -> Result<Vec<SessionSearchResult>, String> {
    run_blocking(app, "search_sessions", move || {
        search_sessions_inner(query, provider_filter)
    })
    .await
}

fn search_sessions_inner(
    query: String,
    provider_filter: Option<String>,
) -> Result<Vec<SessionSearchResult>, String> {
    let query = query.trim().to_string();

    if query.is_empty() {
        return Ok(Vec::new());
    }

    if let Some(provider_id) = provider_filter.as_deref() {
        validate_provider(provider_id)?;
    }

    let mut results = Vec::new();

    for provider in providers::all() {
        if provider_filter
            .as_deref()
            .is_some_and(|provider_id| provider_id != provider.id())
        {
            continue;
        }

        for search_match in provider.search_sessions(&query)? {
            results.push(SessionSearchResult {
                provider: provider.id().to_string(),
                session_id: search_match.session_id,
                snippet: search_match.snippet,
            });
        }
    }

    Ok(results)
}

#[tauri::command]
async fn get_session_history(
    app: tauri::AppHandle,
    provider: String,
    session_id: String,
) -> Result<SessionHistory, String> {
    run_blocking(app, "get_session_history", move || {
        let provider_impl = providers::by_id(&provider)
            .ok_or_else(|| format!("Unsupported provider: {provider}"))?;
        let history = provider_impl.session_history(&session_id)?;

        Ok(SessionHistory {
            provider,
            session_id,
            messages: history
                .messages
                .into_iter()
                .map(|message| SessionMessage {
                    role: message.role,
                    text: message.text,
                })
                .collect(),
            unreadable_lines: history.unreadable_lines,
        })
    })
    .await
}

#[tauri::command]
async fn list_providers(app: tauri::AppHandle) -> Result<Vec<ProviderStatus>, String> {
    run_blocking(app, "list_providers", || Ok(providers::statuses())).await
}

#[tauri::command]
async fn list_repository_branches(
    app: tauri::AppHandle,
    repository_path: String,
) -> Result<Vec<String>, String> {
    run_blocking(app, "list_repository_branches", move || {
        Ok(git_branch_options(&repository_path))
    })
    .await
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, AppState>) -> Result<AppSettings, String> {
    state.db.get_settings()
}

#[tauri::command]
fn save_settings(state: tauri::State<'_, AppState>, settings: AppSettings) -> Result<(), String> {
    if !system_profile::is_rendering_profile(&settings.rendering_profile) {
        return Err("Unsupported rendering profile.".to_string());
    }

    state.db.save_settings(&settings)
}

#[tauri::command]
fn dismiss_rendering_profile_notice(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.dismiss_rendering_profile_notice()
}

#[tauri::command]
fn rename_session(
    state: tauri::State<'_, AppState>,
    provider: String,
    session_id: String,
    friendly_name: String,
) -> Result<(), String> {
    validate_provider(&provider)?;
    validate_friendly_name(&friendly_name)?;
    state
        .db
        .set_friendly_name(&provider, &session_id, &friendly_name)
}

#[tauri::command]
fn set_session_pinned(
    state: tauri::State<'_, AppState>,
    provider: String,
    session_id: String,
    is_pinned: bool,
) -> Result<(), String> {
    validate_provider(&provider)?;
    state
        .db
        .set_session_pinned(&provider, &session_id, is_pinned)
}

#[tauri::command]
fn set_session_discovered_branch(
    state: tauri::State<'_, AppState>,
    provider: String,
    session_id: String,
    branch_name: String,
) -> Result<(), String> {
    validate_provider(&provider)?;
    validate_branch_name(&branch_name)?;
    state
        .db
        .set_session_discovered_branch(&provider, &session_id, &branch_name)
}

#[tauri::command]
fn set_project_favorite(
    state: tauri::State<'_, AppState>,
    working_directory: String,
    is_favorite: bool,
) -> Result<(), String> {
    state
        .db
        .set_project_favorite(&working_directory, is_favorite)
}

#[tauri::command]
fn set_session_collection(
    state: tauri::State<'_, AppState>,
    provider: String,
    session_id: String,
    collection_name: String,
) -> Result<(), String> {
    validate_provider(&provider)?;
    validate_collection_name(&collection_name)?;
    state
        .db
        .set_session_collection(&provider, &session_id, &collection_name)
}

#[tauri::command]
fn set_collection_color(
    state: tauri::State<'_, AppState>,
    collection_name: String,
    color_name: String,
) -> Result<(), String> {
    validate_collection_name(&collection_name)?;
    validate_collection_color(&color_name)?;
    state.db.set_collection_color(&collection_name, &color_name)
}

#[tauri::command]
fn set_session_note(
    state: tauri::State<'_, AppState>,
    provider: String,
    session_id: String,
    note_text: String,
) -> Result<(), String> {
    validate_provider(&provider)?;
    state
        .db
        .set_session_note(&provider, &session_id, &note_text)
}

#[tauri::command]
fn set_session_tags(
    state: tauri::State<'_, AppState>,
    provider: String,
    session_id: String,
    tags: Vec<String>,
) -> Result<(), String> {
    validate_provider(&provider)?;
    validate_tags(&tags)?;
    state.db.set_session_tags(&provider, &session_id, &tags)
}

#[tauri::command]
fn delete_or_hide_session(
    state: tauri::State<'_, AppState>,
    provider: String,
    session_id: String,
) -> Result<DeleteResult, String> {
    let provider_impl =
        providers::by_id(&provider).ok_or_else(|| format!("Unsupported provider: {provider}"))?;
    let settings = state.db.get_settings()?;

    if settings.hard_delete_sessions && provider_impl.supports_delete() {
        provider_impl.delete_session(&session_id)?;
        state.db.delete_indexed_session(&provider, &session_id)?;
        return Ok(DeleteResult {
            action: "deleted".to_string(),
            message: format!(
                "{} session permanently deleted.",
                provider_impl.display_name()
            ),
        });
    }

    state.db.hide_session(&provider, &session_id)?;

    Ok(DeleteResult {
        action: "hidden".to_string(),
        message: "Session hidden from SessionDex.".to_string(),
    })
}

#[tauri::command]
fn unhide_session(
    state: tauri::State<'_, AppState>,
    provider: String,
    session_id: String,
) -> Result<(), String> {
    validate_provider(&provider)?;
    state.db.unhide_session(&provider, &session_id)
}

#[tauri::command]
fn reset_local_data(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.reset_local_data()
}

#[tauri::command]
fn uninstall_app(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<UninstallResult, String> {
    state.db.reset_local_data()?;

    if let Ok(data_dir) = app.path().app_data_dir() {
        remove_database_files(data_dir)?;
    }

    if let Ok(cache_dir) = app.path().app_cache_dir() {
        remove_dir_if_exists(cache_dir)?;
    }

    let app_removal = move_current_app_to_trash()?;
    let app_removal_attempted = app_removal.is_some();
    let app_removed = app_removal.unwrap_or(false);

    let handle = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(1800));
        handle.exit(0);
    });

    let message = if app_removed {
        "SessionDex data was removed and the app was moved to Trash.".to_string()
    } else if app_removal_attempted {
        "SessionDex data was removed. macOS did not allow SessionDex to move the app to Trash; remove the app manually.".to_string()
    } else {
        "SessionDex data was removed. Remove the installed application manually to finish uninstalling."
            .to_string()
    };

    Ok(UninstallResult {
        message,
        app_removal_attempted,
        app_removed,
    })
}

#[tauri::command]
async fn resume_session(
    app: tauri::AppHandle,
    provider: String,
    session_id: String,
    working_directory: Option<String>,
) -> Result<(), String> {
    let db = app.state::<AppState>().db.clone();

    run_blocking(app, "resume_session", move || {
        let provider_impl = providers::by_id(&provider)
            .ok_or_else(|| format!("Unsupported provider: {provider}"))?;
        let settings = db.get_settings()?;
        let command = provider_impl.resume_command(&session_id, working_directory.as_deref());

        terminal::launch(&settings, &command)?;
        db.record_session_resumed(&provider, &session_id)?;

        Ok(())
    })
    .await
}

#[tauri::command]
async fn open_working_directory(app: tauri::AppHandle, path: String) -> Result<(), String> {
    run_blocking(app, "open_working_directory", move || {
        let path = PathBuf::from(path.trim());

        if !path.is_absolute() {
            return Err("Project folder path must be absolute.".to_string());
        }

        if !path.is_dir() {
            return Err(format!("Project folder does not exist: {}", path.display()));
        }

        let status = open_path_command(&path)
            .status()
            .map_err(|err| format!("Failed to open {}: {err}", path.display()))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("Failed to open {}", path.display()))
        }
    })
    .await
}

#[cfg(target_os = "macos")]
fn open_path_command(path: &PathBuf) -> Command {
    let mut command = Command::new("open");
    command.arg(path);
    command
}

#[cfg(target_os = "windows")]
fn open_path_command(path: &PathBuf) -> Command {
    let mut command = Command::new("explorer");
    command.arg(path);
    command
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn open_path_command(path: &PathBuf) -> Command {
    let mut command = Command::new("xdg-open");
    command.arg(path);
    command
}

fn validate_provider(provider: &str) -> Result<(), String> {
    providers::by_id(provider)
        .map(|_| ())
        .ok_or_else(|| format!("Unsupported provider: {provider}"))
}

fn validate_friendly_name(friendly_name: &str) -> Result<(), String> {
    let friendly_name = friendly_name.trim();

    if friendly_name.chars().count() > FRIENDLY_NAME_MAX_CHARS {
        return Err(format!(
            "Custom session names must be {FRIENDLY_NAME_MAX_CHARS} characters or fewer."
        ));
    }

    if friendly_name.chars().any(char::is_control) {
        return Err("Custom session names cannot contain control characters.".to_string());
    }

    Ok(())
}

fn validate_collection_name(collection_name: &str) -> Result<(), String> {
    let collection_name = collection_name.trim();

    if collection_name.chars().count() > COLLECTION_NAME_MAX_CHARS {
        return Err(format!(
            "Collection names must be {COLLECTION_NAME_MAX_CHARS} characters or fewer."
        ));
    }

    if collection_name.chars().any(char::is_control) {
        return Err("Collection names cannot contain control characters.".to_string());
    }

    Ok(())
}

fn validate_collection_color(color_name: &str) -> Result<(), String> {
    match color_name.trim() {
        "" | "none" | "gray" | "red" | "orange" | "yellow" | "green" | "blue" | "purple" => Ok(()),
        value => Err(format!("Unsupported collection color: {value}")),
    }
}

fn validate_tags(tags: &[String]) -> Result<(), String> {
    for tag in tags {
        let normalized_tag = tag.trim().trim_start_matches('#');

        if normalized_tag.is_empty() {
            continue;
        }

        if normalized_tag.chars().count() > TAG_NAME_MAX_CHARS {
            return Err(format!(
                "Tags must be {TAG_NAME_MAX_CHARS} characters or fewer."
            ));
        }

        if !normalized_tag
            .chars()
            .next()
            .is_some_and(|value| value.is_ascii_alphanumeric())
        {
            return Err("Tags must start with a letter or number.".to_string());
        }

        if !normalized_tag.chars().all(|value| {
            value.is_ascii_alphanumeric() || value == '-' || value == '_' || value == '.'
        }) {
            return Err(
                "Tags can use letters, numbers, dashes, underscores, and dots.".to_string(),
            );
        }
    }

    Ok(())
}

fn validate_branch_name(branch_name: &str) -> Result<(), String> {
    let branch_name = branch_name.trim();

    if branch_name.is_empty() {
        return Err("Branch is required.".to_string());
    }

    if branch_name.len() > 200 {
        return Err("Branch names must be 200 characters or fewer.".to_string());
    }

    if branch_name.chars().any(char::is_control) {
        return Err("Branch names cannot contain control characters.".to_string());
    }

    Ok(())
}

fn focus_main_window(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "The SessionDex main window is unavailable.".to_string())?;

    if window.is_minimized().map_err(|error| error.to_string())? {
        window.unminimize().map_err(|error| error.to_string())?;
    }

    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

fn start_instance_dispatcher(app: tauri::AppHandle, owner: InstanceOwner) -> Result<(), String> {
    let InstanceOwner { guard, receiver } = owner;

    thread::Builder::new()
        .name("sessiondex-instance-dispatch".to_string())
        .spawn(move || {
            let _guard = guard;

            loop {
                match receiver.recv() {
                    Ok(InstanceMessage::Launch) => {
                        if let Err(error) = focus_main_window(&app) {
                            eprintln!("SessionDex relaunch failed: {error}");
                            app.exit(1);
                            return;
                        }
                    }
                    Ok(InstanceMessage::Fatal(error)) => {
                        eprintln!("{error}");
                        app.exit(1);
                        return;
                    }
                    Err(error) => {
                        eprintln!("SessionDex instance coordination stopped: {error}");
                        app.exit(1);
                        return;
                    }
                }
            }
        })
        .map(|_| ())
        .map_err(|error| format!("Failed to start SessionDex instance coordination: {error}"))
}

pub fn run() {
    let instance_owner = match instance::claim_or_forward() {
        Ok(InstanceClaim::Primary(owner)) => owner,
        Ok(InstanceClaim::Forwarded) => return,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    let application = tauri::Builder::default()
        .setup(move |app| {
            let db_path = app
                .path()
                .app_data_dir()
                .map_err(std::io::Error::other)?
                .join("sessiondex.sqlite3");
            let db = Database::new(db_path);
            db.init().map_err(std::io::Error::other)?;

            if !db
                .rendering_profile_initialized()
                .map_err(std::io::Error::other)?
            {
                db.initialize_rendering_profile(system_profile::recommended_rendering_profile())
                    .map_err(std::io::Error::other)?;
            }

            app.manage(AppState { db });
            start_instance_dispatcher(app.handle().clone(), instance_owner)
                .map_err(std::io::Error::other)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_sessions,
            refresh_sessions,
            hydrate_session_cards,
            search_sessions,
            get_session_history,
            list_providers,
            list_repository_branches,
            get_settings,
            save_settings,
            dismiss_rendering_profile_notice,
            rename_session,
            set_session_pinned,
            set_session_discovered_branch,
            set_project_favorite,
            set_session_collection,
            set_collection_color,
            set_session_note,
            set_session_tags,
            delete_or_hide_session,
            unhide_session,
            reset_local_data,
            uninstall_app,
            resume_session,
            open_working_directory
        ])
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, WindowEvent::Destroyed) {
                window.app_handle().exit(0);
            }
        })
        .run(tauri::generate_context!());

    if let Err(error) = application {
        eprintln!("SessionDex stopped: {error}");
        std::process::exit(1);
    }
}

fn remove_dir_if_exists(path: PathBuf) -> Result<(), String> {
    match fs::remove_dir_all(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!("Failed to remove {}: {err}", path.display())),
    }
}

fn remove_database_files(data_dir: PathBuf) -> Result<(), String> {
    for file_name in [
        "sessiondex.sqlite3",
        "sessiondex.sqlite3-shm",
        "sessiondex.sqlite3-wal",
    ] {
        let path = data_dir.join(file_name);

        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("Failed to remove {}: {error}", path.display())),
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn move_current_app_to_trash() -> Result<Option<bool>, String> {
    let Some(app_bundle) = current_macos_app_bundle()? else {
        return Ok(None);
    };

    let script = format!(
        "tell application \"Finder\" to delete POSIX file {}",
        applescript_string(&app_bundle.to_string_lossy())
    );

    let status = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .status()
        .map_err(|err| err.to_string())?;

    Ok(Some(status.success()))
}

#[cfg(target_os = "macos")]
fn current_macos_app_bundle() -> Result<Option<PathBuf>, String> {
    let executable = std::env::current_exe().map_err(|err| err.to_string())?;

    for ancestor in executable.ancestors() {
        if ancestor
            .extension()
            .is_some_and(|extension| extension == "app")
        {
            return Ok(Some(ancestor.to_path_buf()));
        }
    }

    Ok(None)
}

#[cfg(target_os = "macos")]
fn applescript_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(not(target_os = "macos"))]
fn move_current_app_to_trash() -> Result<Option<bool>, String> {
    Ok(None)
}
