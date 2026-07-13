# SessionDex Features

SessionDex is a desktop session manager for terminal AI assistant sessions. It is designed for developers who use CLI assistants such as Codex CLI and Claude Code and need a fast way to find, organize, inspect, and resume past sessions without turning the tool into a chat app or cloud service.

This document lists the current implemented features in priority order, from the core product value to supporting operational capabilities.

## 1. Local AI CLI Session Discovery

SessionDex discovers sessions directly from supported local provider history stores.

| Provider | Local history store | Resume command | Current capabilities |
| --- | --- | --- | --- |
| Codex CLI | `~/.codex/sessions` | `codex resume <session_id>` | Discover, search, read history, resume |
| Claude Code | `~/.claude/projects` | `claude --resume <session_id>` | Discover, search, read history, resume |

What this provides:

- Lists sessions from Codex and Claude local JSONL history files with bounded recursive scans.
- Sorts sessions by most recently modified first.
- Shows provider name, session ID, display name, last modified time, and working directory. The display name is the custom SessionDex name when set, otherwise the detected provider title or raw session ID.
- Exposes the generated resume command for copying.
- Tolerates missing provider directories and missing provider CLIs.
- Still shows discoverable history files when a provider CLI is missing, while disabling Resume for that provider until the CLI is available.

Why it matters:

Developer AI sessions often live in provider-specific folders and are hard to scan after a few days of work. SessionDex makes those local session stores visible in one desktop dashboard.

## 2. Local-First Data Safety

SessionDex is intentionally offline-first and stores only app-owned metadata.

SessionDex-owned data includes:

- Custom session names.
- Hidden sessions.
- Pinned sessions.
- Recent resume timestamps.
- Favorite projects.
- Discovered Git repository and branch snapshots.
- Collections and collection colors.
- Notes.
- Tags.
- Settings.

Provider-owned data remains with the original CLI tools.

SessionDex does not:

- Rewrite Codex or Claude history during discovery, search, history reading, or resume.
- Copy full provider transcripts into the SessionDex SQLite database.
- Make app-runtime network calls, send telemetry or analytics, use embeddings, add cloud sync, or call external search services.
- Delete provider histories for built-in providers that do not support deletion.

Why it matters:

The app can add organization and retrieval features without taking ownership of provider transcripts or creating another sensitive data store.

## 3. Resume Sessions In The Right Terminal Context

SessionDex can reopen a selected session in the user's terminal instead of requiring manual command lookup.

What this provides:

- Opens Codex sessions with `codex resume <session_id>`.
- Opens Claude sessions with `claude --resume <session_id>`.
- Changes into the session working directory before running the resume command when a working directory was discovered.
- Records `lastResumed` in SessionDex metadata so recently resumed sessions can be filtered later.
- Allows copying the generated resume command without launching it.
- Allows copying the raw provider session ID.

Terminal support:

- macOS: Terminal by default, with iTerm/iTerm2 support through the terminal setting.
- Linux: auto-detects common terminals such as `x-terminal-emulator`, GNOME Terminal, KDE Konsole, Xfce Terminal, Mate Terminal, Tilix, Alacritty, Kitty, WezTerm, and XTerm.
- Windows: auto-detects Windows Terminal, PowerShell, or Command Prompt.
- Advanced setting: users can provide a terminal executable override.

Why it matters:

The value of a session manager is not only finding an old session, but returning to it quickly with the right CLI and project directory.

## 4. Search Across Metadata And Chat History

SessionDex searches both app-owned metadata and provider-owned chat history.

What this provides:

- Searches custom session names, session IDs, providers, notes, tags, collections, working directories, folder names, discovered repositories, and discovered branches.
- Searches readable user and assistant text from provider JSONL files on demand.
- Displays chat-history snippets for matching sessions.
- Supports provider-aware history search to avoid scanning unrelated providers when a provider filter is active.
- Does not store full transcripts in the SessionDex database.
- Does not use cloud search, embeddings, telemetry, or network services.

Supported search filters:

| Filter | Example | Behavior |
| --- | --- | --- |
| Plain text | `migration plan` | Searches metadata and readable chat text. |
| `provider:` | `provider:codex` | Limits results to a provider ID such as `codex` or `claude`. |
| `collection:` | `collection:release` | Matches sessions whose collection contains the value. |
| `tag:` | `tag:postgres` | Requires the normalized tag to be present. Multiple tag filters are all required. |
| `pinned:` | `pinned:true` | Filters pinned or unpinned sessions. |
| `hidden:` | `hidden:true` | Filters hidden or visible sessions. |
| `folder:` | `folder:session-dex` | Matches the working directory or folder name. |
| `after:` | `after:2026-07-01` | Keeps sessions modified on or after the date. |
| `before:` | `before:2026-08` | Keeps sessions modified before the date. |

Quoted values are supported, for example `collection:"Production Issues"` or `folder:"My Project"`.

Why it matters:

Session titles and IDs are often not enough. Searching the actual conversation text, notes, tags, and project context makes old assistant work recoverable.

## 5. Chat History Reading And Session Previews

SessionDex can inspect readable chat content from provider session files without importing the full transcript into its own database.

What this provides:

- Session cards show a compact clickable preview of the first user input and the latest readable user or assistant message when available.
- A full history modal displays parsed user and assistant messages for the selected session.
- Fenced code blocks and inline code are rendered distinctly in the history view.
- Non-text content such as tool, image, audio, and file payloads is skipped.
- Unreadable or unparsable JSONL lines are counted and reported instead of failing the whole history view.
- Keyboard navigation is available inside the history modal.

Why it matters:

Developers need enough context to identify the right session before resuming it. SessionDex provides that context without becoming a separate transcript database.

## 6. Local Session Organization

SessionDex stores user-managed organization metadata in its own local SQLite database.

What this provides:

- Custom session names.
- Pinned sessions.
- Collections.
- Collection colors: gray, red, orange, yellow, green, blue, and purple.
- Session notes.
- Tags.
- Favorite project markers by working directory.
- Hidden sessions.
- Recently resumed timestamps.

Important behavior:

- Blank custom names remove the custom SessionDex name.
- Custom session names are limited to 100 characters.
- Blank collections remove the session from its collection.
- Collection names are limited to 48 characters.
- Blank notes remove the note.
- Tags entered through the UI are normalized to lowercase, must start with a letter or number, and can then use letters, numbers, dashes, underscores, and dots.
- Tags are limited to 32 characters each.
- Project favorites apply by working directory, so sessions from the same project show the same favorite marker.

Why it matters:

Provider session stores are optimized for the CLI provider. SessionDex adds a local organization layer without modifying provider histories.

## 7. Dashboard Views And Filters

The main screen is a dense dashboard built for repeated session triage.

What this provides:

- All sessions view.
- Pinned sessions view.
- Provider filter.
- Collection filter, including an Unassigned option when applicable.
- Activity filter:
  - Any time
  - Today
  - Yesterday
  - Last 7 days
  - Last month
  - Recently resumed
- Hidden-session visibility controlled through Settings.
- Pinned sessions are promoted to the top of the All view.
- Recently resumed view sorts by resume time.
- Empty states explain why no sessions are visible under the active filters.

Why it matters:

Session recovery is usually a filtering problem. SessionDex provides direct controls for provider, activity, collection, and importance, while keeping project and folder context searchable.

## 8. Provider Status And Compatibility Visibility

SessionDex makes provider availability visible without requiring providers to be installed before the app can open.

What this provides:

- Settings shows detected AI CLIs.
- Settings shows each provider's display name, executable, and Detected or Missing state.
- The backend tracks session-store existence, deletion support, and capability labels for provider integrations.
- Missing CLIs are shown as missing rather than treated as fatal app errors.
- Session cards show when Resume is unavailable because the corresponding CLI is not detected.

Setup diagnostics warn when optional provider versions are below:

- Codex CLI: `>= 0.144.1`
- Claude Code: `>= 2.1.187`

Why it matters:

SessionDex is useful even while a provider is temporarily unavailable, and it gives clear information about what is missing.

## 9. Git Repository And Branch Context

SessionDex records best-effort Git context for sessions with a discovered working directory.

What this provides:

- Detects the repository root from the working directory when SessionDex first discovers a session.
- Records the current Git branch at first discovery when available.
- Shows discovered branch context on the session card.
- Allows updating the session branch with a searchable suggestion dropdown.
- Offers local branch suggestions from `refs/heads` and `refs/remotes` for the detected repository.

Important behavior:

- Branch discovery is a SessionDex metadata snapshot, not a provider transcript edit.
- Updating the session branch does not switch the Git checkout.
- The branch value is best-effort and depends on the working directory and Git state when SessionDex first sees the session.

Why it matters:

Developers often remember work by repository and branch. Capturing that context makes older sessions easier to identify and resume.

## 10. Safe Hide And Delete Model

SessionDex separates provider deletion from local hiding.

What this provides:

- If a provider supports safe deletion, SessionDex calls the provider's delete operation.
- If a provider does not support deletion, SessionDex hides the session from its own dashboard.
- Hidden sessions can be shown from Settings and restored with Unhide.
- Current built-in Codex and Claude providers do not advertise deletion support, so their sessions are hidden rather than deleted.

Why it matters:

Provider histories are user data. SessionDex avoids destructive behavior unless the provider integration explicitly supports it.

## 11. Command Palette And Keyboard Workflow

SessionDex includes a command palette and keyboard shortcuts for fast operation.

What this provides:

- Command palette with `Cmd/Ctrl+Shift+P`.
- Search focus with `Cmd/Ctrl+K`.
- Browser/WebView find is intercepted with `Cmd/Ctrl+F`; use `Cmd/Ctrl+K` for SessionDex search.
- Settings with `Cmd/Ctrl+,`.
- Refresh with `F5` or `Cmd/Ctrl+R`.
- Switch All/Pinned views with `Cmd/Ctrl+1` and `Cmd/Ctrl+2`.
- Arrow-key navigation across session cards.
- Home/End navigation to first or last session.
- Enter or Space to open selected chat history.
- `Cmd/Ctrl+Enter` to resume the selected session.
- `F2` to rename the selected session.
- `P` to pin or unpin.
- `C` to set collection.
- `N` to edit notes.
- `T` to edit tags.
- `Delete` to delete or hide.
- `U` to unhide a selected hidden session.
- `Esc` to close dialogs or clear search.

The command palette also includes selected-session actions. When a palette query is entered, it adds resume, history, and select actions for matching sessions.

Why it matters:

Frequent users can move through sessions without switching constantly between mouse, keyboard, and terminal.

## 12. Refresh, Status Messages, And Statistics

SessionDex keeps session data current while staying explicit about user actions.

What this provides:

- Manual refresh from the header or keyboard.
- Automatic refresh every 12 seconds while the app window is visible and focused.
- Background refresh avoids overlapping refresh work.
- Success and error messages for resume, rename, collection, note, tag, hide, unhide, copy, and folder actions.
- Toasts for copied resume commands and session IDs.
- Statistics popover with total sessions, provider counts, pinned sessions, hidden sessions, collection count, and tag count.

Why it matters:

AI CLI sessions can be created outside SessionDex. Refresh and status feedback keep the dashboard trustworthy without hiding failures.

## 13. Settings

SessionDex includes a compact settings panel for local preferences and diagnostics.

What this provides:

- Light and dark theme selection.
- Detected or Missing status for built-in provider CLIs.
- Terminal executable override.
- Hidden session visibility toggle.
- Built-in keyboard shortcut reference.

Persisted settings:

- Theme.
- Terminal executable override.
- Provider filter.
- Show hidden sessions.

Why it matters:

The app should work with sensible defaults while still allowing users to adapt terminal behavior and visibility preferences.

## 14. Project Folder Actions

SessionDex can open a session's detected project folder in the operating system file manager.

What this provides:

- Opens folders with `open` on macOS.
- Opens folders with `xdg-open` on Linux.
- Opens folders with Explorer on Windows.
- Validates that the path is absolute and exists as a directory before opening it.

Why it matters:

Resuming a session often goes together with inspecting the project files that produced it.

## 15. Source-Build Installation And Updates

SessionDex currently ships as a source-build desktop app.

What this provides:

- macOS and Linux installer script: `./install.sh`.
- Windows installer wrapper: `.\install.cmd`.
- Direct Windows PowerShell installer: `.\install.ps1`.
- Prerequisite checks before building.
- Optional prerequisite installation prompts when supported by the platform.
- `npm ci` dependency installation.
- Native Tauri package build for the current operating system.
- Prompted installation of the generated app.
- Update support by pulling new source and rerunning the same installer while preserving app-owned data.
- Running-app detection before installing over an existing app.
- Prompt before force-closing SessionDex if it does not exit cleanly.

Generated package targets:

- macOS: `.app` and `.dmg`.
- Debian-family Linux: `.deb`.
- Other Linux distributions: `.AppImage`.
- Windows: NSIS installer.

Why it matters:

The source-build model keeps early distribution transparent and avoids unsigned installer, notarization, auto-update, and package publishing commitments before the app is stable.

## 16. Prerequisite Diagnostics

SessionDex includes setup diagnostics for build requirements and optional providers.

What this provides:

- Checks Git, Node.js, npm, Rust, and Cargo.
- Checks macOS Xcode Command Line Tools.
- Checks Linux native Tauri build dependencies such as compiler tools, WebKitGTK, OpenSSL, libxdo, AppIndicator, librsvg, pkg-config, and Debian `fakeroot` when needed.
- Checks Windows MSVC Rust toolchain, Visual C++ Build Tools, and WebView2 Runtime.
- Checks optional Codex CLI and Claude Code versions and session directories.
- Prints platform-specific remediation commands for missing prerequisites.

Why it matters:

Source builds fail for predictable reasons. The diagnostics turn those failures into concrete setup actions.

## 17. Uninstall And Local Data Removal

SessionDex includes uninstall scripts for removing the installed app and, optionally, app-owned data.

What this provides:

- macOS/Linux uninstaller: `./uninstall.sh`.
- Windows uninstaller wrapper: `.\uninstall.cmd`.
- Direct Windows PowerShell uninstaller: `.\uninstall.ps1`.
- Running-app detection before uninstalling.
- Removal from common install locations.
- Non-interactive app removal while keeping data with `--yes`, `-y`, or `-Yes`.
- Optional SessionDex data removal with `--delete-data` or `-DeleteData`.
- Clear explanation that Codex and Claude history files are not touched.

Why it matters:

Users can remove the application without accidentally deleting provider-owned chat history or local SessionDex metadata unless they explicitly choose to delete it.

## 18. Desktop App Foundation

SessionDex is implemented as a Tauri v2 desktop application with a React and TypeScript frontend.

What this provides:

- Native desktop shell through Tauri v2.
- React 19 frontend.
- TypeScript strict mode.
- Vite build pipeline.
- Tailwind CSS v4 styling.
- Local SQLite metadata through `rusqlite` with bundled SQLite.
- Provider integrations behind a `SessionProvider` trait and registry.
- Minimal Tauri capability configuration using `core:default`.

Why it matters:

The architecture keeps the app local, portable, and straightforward to extend with additional local providers later.

## Product Boundaries

SessionDex is intentionally focused.

It is not:

- An AI assistant.
- A chat app.
- A code editor.
- An analytics dashboard.
- A cloud sync product.
- A transcript warehouse.

The product goal is narrow: help developers find, organize, inspect, hide, and resume local terminal AI assistant sessions.
