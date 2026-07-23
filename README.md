# SessionDex

SessionDex is a lightweight desktop session manager for terminal AI assistants like Codex CLI and Claude Code.

It is not an AI assistant, chat app, code editor, analytics dashboard, or cloud product. Its only job is to help you find, rename, hide/delete, and resume previous CLI sessions.

## Features

See [FEATURES.md](FEATURES.md) for the full feature list, including provider support, search, resume behavior, local metadata, privacy boundaries, keyboard workflow, install/update support, and uninstall behavior.

## Current Distribution Model

SessionDex is currently source-build only.

This is intentional for early versions. SessionDex is a developer tool, and source-build distribution keeps installation transparent while avoiding unsigned installer warnings, notarization, auto-update, and package publishing work before the app is stable.

## Install From Source

### macOS and Linux

```bash
git clone https://github.com/tonyghouse/session-dex.git
cd session-dex
./install.sh
```

### Windows

```powershell
git clone https://github.com/tonyghouse/session-dex.git
cd session-dex
.\install.cmd
```

You can also run the PowerShell installer directly:

```powershell
.\install.ps1
```

The installer:

- checks required build prerequisites
- prints clear missing-prerequisite actions
- asks before running available prerequisite installers
- runs `npm ci`
- builds the native desktop package for your OS
- asks whether to install the generated app
- asks before closing a running SessionDex app during installation or update

Generated packages are created under:

```text
src-tauri/target/release/bundle
```

## Minimum Build Requirements

| Component | Minimum | Recommended / notes |
| --- | --- | --- |
| Git | Any recent version | Required for cloning and updating from source. |
| Node.js | `>= 20.19.0` | Node.js 22 LTS is recommended. |
| npm | `>= 10.0.0` | Use the npm bundled with Node.js LTS. |
| Rust | `>= 1.77.2` | Latest stable Rust via `rustup` is recommended. |
| Windows Rust toolchain | `stable-msvc` | GNU Rust toolchains are not supported for this Tauri build. |
| Tauri CLI | No global install | The repo uses local `@tauri-apps/cli` from `npm ci`. |
| Tauri | v2 | Current repo versions: `@tauri-apps/cli 2.11.4`, Rust `tauri 2.11.5`. |

You do not need to install Tauri globally.

## Optional Provider Compatibility

Codex CLI and Claude Code are optional runtime integrations. SessionDex can be built and opened without either CLI installed. Missing providers simply show as not detected.

| Provider | Executable | Minimum supported version | Expected history path | Resume command |
| --- | --- | --- | --- | --- |
| Codex CLI | `codex` | `>= 0.144.1` | `~/.codex/sessions/**/*.jsonl` | `codex resume <session_id>` |
| Claude Code | `claude` | `>= 2.1.187` | `~/.claude/projects/**/*.jsonl` | `claude --resume <session_id>` |

Older CLI versions may work if they use the same JSONL history layout, but they are outside the supported compatibility matrix.

## OS Build Prerequisites

The install scripts check these automatically. The commands below are here so failures are easy to understand and fix manually.

### macOS

Install Apple Command Line Tools:

```bash
xcode-select --install
```

Install Node.js 22 LTS from `https://nodejs.org/`, or with Homebrew:

```bash
brew install node
```

Install Rust:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Ubuntu, Debian, Linux Mint, Pop!_OS

```bash
sudo apt update
sudo apt install -y git build-essential curl wget file fakeroot pkg-config libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

Install Node.js 22 LTS from `https://nodejs.org/` or your preferred version manager if your distro package is older than `20.19.0`.

Install Rust:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Fedora

```bash
sudo dnf check-update
sudo dnf group install -y c-development
sudo dnf install -y git curl wget file pkgconf-pkg-config webkit2gtk4.1-devel openssl-devel libxdo-devel libappindicator-gtk3-devel librsvg2-devel
```

Install Node.js 22 LTS from `https://nodejs.org/` or your preferred version manager if your distro package is older than `20.19.0`.

Install Rust:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### RHEL, Rocky Linux, AlmaLinux, CentOS

```bash
sudo dnf group install -y "Development Tools"
sudo dnf install -y git curl wget file pkgconf-pkg-config webkit2gtk4.1-devel openssl-devel libxdo-devel libappindicator-gtk3-devel librsvg2-devel
```

If WebKitGTK packages are unavailable, enable the appropriate CRB/EPEL repositories for your distribution and retry.

Install Node.js 22 LTS from `https://nodejs.org/` or your preferred version manager if your distro package is older than `20.19.0`.

Install Rust:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Arch Linux, Manjaro

```bash
sudo pacman -Syu
sudo pacman -S --needed git base-devel curl wget file pkgconf webkit2gtk-4.1 openssl libxdo libayatana-appindicator librsvg
```

Install Node.js 22 LTS and Rust with your preferred Arch packages or toolchain manager.

### openSUSE

```bash
sudo zypper refresh
sudo zypper install -t pattern devel_basis
sudo zypper install git curl wget file pkg-config webkit2gtk3-devel libopenssl-devel libxdo-devel libappindicator3-devel librsvg-devel
```

Install Node.js 22 LTS from `https://nodejs.org/` or your preferred version manager if your distro package is older than `20.19.0`.

Install Rust:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Windows

Installers use `winget` when you allow prerequisite installation. Manual commands:

```powershell
winget install --id Git.Git -e --source winget
winget install --id OpenJS.NodeJS.LTS -e --source winget
winget install --id Rustlang.Rustup -e --source winget
rustup default stable-msvc
winget install --id Microsoft.EdgeWebView2Runtime -e --source winget
```

Install Microsoft C++ Build Tools with the Desktop development with C++ workload:

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --source winget --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

Windows Terminal is recommended for resuming sessions. SessionDex also falls back to PowerShell or Command Prompt.

## Manual Build Commands

If you do not want to use the installer scripts:

```bash
npm ci
```

Build for the current OS:

```bash
npm run build
```

Build a specific package:

```bash
# macOS
npm run build -- --bundles app,dmg

# Ubuntu/Debian
npm run build -- --bundles deb

# Other Linux
npm run build -- --bundles appimage

# Windows
npm run build -- --bundles nsis
```

On Ubuntu/Debian, Debian package builds use `fakeroot` so package metadata records files as root-owned `0:0`. AppImage-only builds do not need `fakeroot`.

## Update From Source

The same installer scripts handle updates. Pull the latest source, rerun the installer, and follow the prompts.

```bash
git pull --ff-only
./install.sh
```

On Windows:

```powershell
git pull --ff-only
.\install.cmd
```

If SessionDex is running when the generated app is about to be installed, the installer asks whether to close it. If the app does not exit cleanly, the installer asks again before force-closing it.

This update model works as long as SessionDex keeps backward-compatible app data and database migrations. The project should treat this as a compatibility contract.

## Uninstall

Removing the app package removes the application, but operating systems usually keep app data separately. SessionDex stores local metadata separately from the installed app so normal updates do not delete your custom session names, hidden sessions, pinned sessions, or settings.

### macOS and Linux

```bash
./uninstall.sh
```

### Windows

```powershell
.\uninstall.cmd
```

You can also run the PowerShell uninstaller directly:

```powershell
.\uninstall.ps1
```

The uninstaller:

- removes the installed SessionDex app from common install locations
- asks before closing a running SessionDex app
- later shows a `[y/N]` prompt before deleting preferences and app data
- keeps preferences and app data when you press Enter at that prompt

SessionDex-owned data includes `sessiondex.sqlite3`, settings, custom names, hidden/pinned sessions, and cache. Codex and Claude session history is not touched.

For non-interactive app removal while keeping preferences/data:

```bash
./uninstall.sh --yes
```

On Windows:

```powershell
.\uninstall.ps1 -Yes
```

To also delete SessionDex preferences and app data:

```bash
./uninstall.sh --yes --delete-data
```

On Windows:

```powershell
.\uninstall.ps1 -Yes -DeleteData
```

The old macOS helper still works and delegates to the main uninstaller:

```bash
bash scripts/uninstall-macos.sh
```

## Data Safety Contract

SessionDex stores only its own metadata.

It does not rewrite CLI session history.

SessionDex-owned data includes:

- custom session names
- hidden sessions
- pinned sessions
- recent resume timestamps
- favorite projects
- discovered Git repository and branch snapshots
- collections and collection colors
- notes
- tags
- settings

Provider-owned data stays with the original CLI tool.

## App Icon Assets

The editable source icon is `src-tauri/icons/icon.svg`.

Desktop installers still need generated platform icon files:

- macOS uses `icon.icns`
- Linux uses PNG icon sizes
- Windows desktop uses `icon.ico`

Android, iOS, and Windows Store icon outputs are intentionally not kept in this repo because SessionDex is a desktop app.

## Usage

Open SessionDex after installing it.

SessionDex runs as one foreground GUI process per signed-in user. Launching it again focuses the existing window, and closing the window exits the process completely. SessionDex does not use folder-based launch arguments, so commands such as `sessiondex .` or `session-dex .` are not part of its workflow.

The dashboard shows detected sessions from supported providers. Provider connection status is available in Settings under Detected AI CLIs.

SessionDex reads provider session files directly when the app loads and when you click Refresh. Unchanged session-card metadata is cached only in process memory to keep refreshes lightweight. It does not copy provider sessions into its own database.

Resume opens the selected session in Terminal on macOS, an auto-detected terminal emulator on Linux, and Windows Terminal, PowerShell, or Command Prompt on Windows.

For each session you can:

- see a short preview of the first user input and latest user input when the provider history format exposes it
- resume the session in your configured terminal
- rename it locally inside SessionDex
- delete it if the provider supports deletion
- hide it from SessionDex if provider deletion is not supported
- show hidden sessions from Settings and unhide them later

Search matches custom session names, session ids, providers, notes, tags, collections, working directories, discovered Git context, and local chat history. Chat-history search scans provider session files on demand and matches extracted user and assistant message text. It does not copy full transcripts into SessionDex SQLite, use embeddings, or call any cloud service.

## Philosophy

Simple.

Fast.

Reliable.

Offline-first.

Focused on doing one thing well.
