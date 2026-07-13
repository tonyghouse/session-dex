# SessionDex Agent Instructions

These instructions apply to this repository. They supplement the global rules from the user; if anything conflicts, follow the user-level or conversation-specific instruction.

## Project Shape

- SessionDex is a local-first Tauri v2 desktop app for finding, renaming, hiding/deleting, and resuming terminal AI assistant sessions.
- The frontend lives in `src/` and uses React, TypeScript, Vite, Tailwind CSS v4, and small local UI primitives in `src/components/ui/`.
- The native app lives in `src-tauri/src/` and uses Rust, Tauri commands, provider adapters, terminal launching, and a small SQLite metadata database via `rusqlite`.
- `src/lib/api.ts` is the frontend boundary for Tauri invokes. Keep it aligned with Rust `#[tauri::command]` functions and shared shapes in `src/lib/types.ts` and `src-tauri/src/models.rs`.
- Provider integrations live under `src-tauri/src/providers/` and implement the `SessionProvider` trait in `providers/mod.rs`.

## Product Constraints

- Keep SessionDex focused: it is not a chat app, code editor, analytics dashboard, cloud sync product, or AI assistant.
- Preserve the offline-first model. Do not add network calls, telemetry, embeddings, analytics, or cloud services unless explicitly requested.
- Preserve the data-safety contract: SessionDex stores only its own metadata such as friendly names, hidden sessions, pinned sessions, recent resume timestamps, favorite projects, discovered Git repository and branch snapshots, collections and collection colors, notes, tags, and settings.
- Do not copy full provider transcripts into the app database. Provider-owned histories must remain with the original CLI tools.
- Be careful around provider session files. Reading/parsing is expected; rewriting provider histories is not.
- Deletion support is provider-specific. If a provider does not support deletion, hide the session in SessionDex metadata instead.

## Coding Conventions

- Prefer the smallest scoped change that preserves the current architecture.
- Use TypeScript strict types and keep frontend data models in camelCase.
- Rust models exposed to the frontend should use `#[serde(rename_all = "camelCase")]`.
- Keep command names and invoke argument names stable unless you update both frontend and Rust call sites together.
- Use existing local utilities such as `cn`, `formatModifiedTime`, `Button`, `Input`, `Badge`, and `Card` before adding new helpers or UI primitives.
- Use lucide-react icons for app controls when an icon is needed.
- Keep the UI dashboard-first, dense, and task-oriented. Avoid marketing-page patterns or decorative layouts.
- Keep comments rare and practical; add them only where they clarify non-obvious parsing, terminal, database, or platform behavior.
- Do not edit generated or build output such as `dist/`, `src-tauri/target/`, `src-tauri/gen/`, or `node_modules/`.
- Treat `src-tauri/icons/icon.svg` as the editable source icon. Generated platform icon files should be regenerated intentionally, not hand-edited.

## Dependency And Package Rules

- Use npm for this project. Keep `package-lock.json` in sync when dependencies change.
- Do not switch package managers or introduce a new frontend framework.
- Avoid new dependencies unless they remove real complexity or are needed for a requested feature.
- Do not install global software or system packages as part of normal repo work.

## Verification

Use the lightest command that proves the change:

- `npm run check` for TypeScript typechecking.
- `npm run frontend:build` for frontend typecheck plus Vite build.
- `cd src-tauri && cargo test` for Rust unit tests.
- `npm run build` for a full Tauri desktop build.
- `npm run build:linux:deb` only when Debian packaging specifically needs verification.

Notes:

- Full Tauri builds may require Rust, Tauri OS dependencies, and platform-specific tooling.
- On Linux, `.deb` builds require `fakeroot`; `scripts/build.mjs` handles invoking it when a deb bundle is requested.
- There is no lint script currently configured. Do not claim lint verification unless one is added and run.

## Build And Distribution

- The current distribution model is source-build only.
- Do not add signing, notarization, auto-update, release publishing, or installer automation unless explicitly requested.
- Generated packages are expected under `src-tauri/target/release/bundle`.
- macOS packages may be unsigned/not notarized during early source-build distribution.

## Documentation And Release Notes

- Treat `FEATURES.md` as the authoritative inventory of implemented user-facing features, not a roadmap or marketing wish list.
- Keep `README.md` concise and installation-focused. It should link to `FEATURES.md` instead of duplicating the full feature inventory.
- When changing user-facing behavior, provider support, search/filter syntax, metadata fields, keyboard shortcuts, terminal behavior, install/update scripts, uninstall behavior, or distribution targets, update `FEATURES.md` and any affected README summary in the same change.
- Verify release-facing documentation against the implementation before finalizing it. Check the relevant source of truth: `src/App.tsx` for UI and shortcuts, `src/lib/api.ts` and `src/lib/types.ts` for frontend/backend shapes, `src-tauri/src/lib.rs` for Tauri commands, `src-tauri/src/providers/` for provider capabilities, `src-tauri/src/terminal.rs` for resume/open behavior, `src-tauri/src/db.rs` for persisted metadata, and install/uninstall/doctor scripts for setup claims.
- Do not document future providers, cloud features, telemetry, embeddings, signing/notarization, auto-update, app-store distribution, or provider-history deletion unless the repository implements them.
- For release-readiness doc audits, report what was checked, what changed, and which verification commands were run.

## Provider Integration Guidance

- Add new providers by implementing `SessionProvider`, registering them in `providers::all()`, and keeping status/search/history/resume behavior consistent with Codex and Claude.
- Session discovery should tolerate missing provider directories and unreadable individual files.
- Search should parse provider histories on demand and return snippets, not persist transcript content.
- Resume commands should flow through `ResumeCommand` and `terminal::launch` so shell escaping and platform terminal behavior stay centralized.
- Preserve depth limits and defensive parsing patterns unless the provider format requires a documented change.

## Database Guidance

- The SQLite database is app-owned metadata. Schema changes belong in `src-tauri/src/db.rs`.
- Make schema updates backward-compatible for existing source-build users when possible.
- Keep reset/uninstall behavior limited to SessionDex-owned data and cache.
- Do not delete or mutate Codex or Claude data during uninstall/reset flows.

## Git And Review Safety

- Do not modify unrelated files.
- Do not commit, push, create PRs, merge PRs, or rewrite history unless explicitly asked.
- Before finishing, report what changed and which verification commands were run. If verification was skipped, say why.
