#!/usr/bin/env bash

set -euo pipefail

ASSUME_YES=0
DELETE_DATA=0

usage() {
  cat <<'EOF'
SessionDex uninstaller

Usage:
  ./uninstall.sh [--yes] [--delete-data]

Options:
  --yes          Skip app-removal prompts. This does not delete preferences/data.
  --delete-data  Also delete SessionDex preferences, cache, and sessiondex.sqlite3.
  -h, --help     Show this help.

By default, SessionDex preferences and app data are kept.
EOF
}

for arg in "$@"; do
  case "$arg" in
    --yes|-y)
      ASSUME_YES=1
      ;;
    --delete-data)
      DELETE_DATA=1
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      printf "Unknown option: %s\n\n" "$arg" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -t 1 && -z "${NO_COLOR:-}" ]]; then
  BOLD="$(printf '\033[1m')"
  GREEN="$(printf '\033[32m')"
  YELLOW="$(printf '\033[33m')"
  RED="$(printf '\033[31m')"
  RESET="$(printf '\033[0m')"
else
  BOLD=""
  GREEN=""
  YELLOW=""
  RED=""
  RESET=""
fi

prompt_yes_no() {
  local prompt="$1"
  local default="$2"
  local answer

  if (( ASSUME_YES )); then
    printf "%s yes\n" "$prompt"
    return 0
  fi

  if [[ "$default" == "yes" ]]; then
    read -r -p "$prompt [Y/n] " answer
    [[ -z "$answer" || "$answer" =~ ^[Yy]$ ]]
  else
    read -r -p "$prompt [y/N] " answer
    [[ "$answer" =~ ^[Yy]$ ]]
  fi
}

confirm_delete_data() {
  if (( DELETE_DATA )); then
    printf "Preference/data deletion enabled by --delete-data.\n"
    return 0
  fi

  if (( ASSUME_YES )); then
    printf "Keeping SessionDex preferences and app data. Use --delete-data to remove them.\n"
    return 1
  fi

  prompt_yes_no "Delete SessionDex preferences and app data, including sessiondex.sqlite3?" "no"
}

running_app_pids() {
  case "$(uname -s)" in
    Darwin)
      pgrep -x "SessionDex" 2>/dev/null || true
      ;;
    Linux)
      {
        pgrep -x "sessiondex" 2>/dev/null || true
        pgrep -x "SessionDex" 2>/dev/null || true
      } | sort -u
      ;;
  esac
}

wait_for_app_exit() {
  local seconds="$1"
  local elapsed=0

  while (( elapsed < seconds )); do
    if [[ -z "$(running_app_pids)" ]]; then
      return 0
    fi

    sleep 1
    elapsed=$((elapsed + 1))
  done

  [[ -z "$(running_app_pids)" ]]
}

request_close_running_app() {
  if [[ -z "$(running_app_pids)" ]]; then
    return 0
  fi

  printf "%sSessionDex is currently running.%s\n" "$YELLOW" "$RESET"

  if ! prompt_yes_no "Close SessionDex before uninstalling?" "yes"; then
    printf "Uninstall skipped. Close SessionDex and rerun this script.\n"
    return 1
  fi

  case "$(uname -s)" in
    Darwin)
      if command -v osascript >/dev/null 2>&1; then
        osascript -e 'tell application "SessionDex" to quit' >/dev/null 2>&1 || true
      fi
      ;;
    Linux)
      pkill -TERM -x "sessiondex" 2>/dev/null || true
      pkill -TERM -x "SessionDex" 2>/dev/null || true
      ;;
  esac

  if wait_for_app_exit 15; then
    return 0
  fi

  printf "%sSessionDex did not close within 15 seconds.%s\n" "$YELLOW" "$RESET"

  if ! prompt_yes_no "Force close SessionDex now?" "no"; then
    printf "Uninstall skipped. Close SessionDex and rerun this script.\n"
    return 1
  fi

  case "$(uname -s)" in
    Darwin)
      pkill -KILL -x "SessionDex" 2>/dev/null || true
      ;;
    Linux)
      pkill -KILL -x "sessiondex" 2>/dev/null || true
      pkill -KILL -x "SessionDex" 2>/dev/null || true
      ;;
  esac

  if wait_for_app_exit 5; then
    return 0
  fi

  printf "%sSessionDex is still running. Uninstall skipped.%s\n" "$RED" "$RESET"
  return 1
}

run_privileged() {
  if [[ "$(id -u)" -eq 0 ]]; then
    "$@"
  elif command -v sudo >/dev/null 2>&1; then
    sudo "$@"
  else
    printf "%sAdministrator permissions are required for:%s %q" "$YELLOW" "$RESET" "$1"
    shift
    printf " %q" "$@"
    printf "\n"
    return 1
  fi
}

remove_path() {
  local path="$1"
  local label="$2"

  if [[ -z "$path" ]]; then
    return 0
  fi

  if [[ ! -e "$path" && ! -L "$path" ]]; then
    printf "Not found: %s\n" "$path"
    return 0
  fi

  if rm -rf "$path" 2>/dev/null; then
    printf "Removed %s: %s\n" "$label" "$path"
    return 0
  fi

  printf "%sCould not remove %s without elevated permissions.%s\n" "$YELLOW" "$path" "$RESET"
  if prompt_yes_no "Use sudo to remove it now?" "no"; then
    run_privileged rm -rf "$path"
    printf "Removed %s: %s\n" "$label" "$path"
  else
    printf "Left in place: %s\n" "$path"
  fi
}

find_debian_package() {
  local package status
  for package in sessiondex session-dex io.sessiondex.desktop; do
    status="$(dpkg-query -W -f='${Status}' "$package" 2>/dev/null || true)"
    if [[ "$status" == "install ok installed" ]]; then
      printf "%s" "$package"
      return 0
    fi
  done

  return 1
}

remove_debian_package() {
  if ! command -v dpkg-query >/dev/null 2>&1; then
    return 0
  fi

  local package
  package="$(find_debian_package || true)"
  if [[ -z "$package" ]]; then
    printf "No installed Debian package found for SessionDex.\n"
    return 0
  fi

  printf "Removing Debian package: %s\n" "$package"
  if command -v apt >/dev/null 2>&1; then
    run_privileged apt remove -y "$package"
  else
    run_privileged dpkg -r "$package"
  fi
}

uninstall_macos_app() {
  printf "\n%sRemoving macOS app%s\n" "$BOLD" "$RESET"
  remove_path "$HOME/Applications/SessionDex.app" "app"
  remove_path "/Applications/SessionDex.app" "app"
}

uninstall_linux_app() {
  printf "\n%sRemoving Linux app%s\n" "$BOLD" "$RESET"
  remove_debian_package
  remove_path "$HOME/.local/bin/SessionDex.AppImage" "AppImage"
  remove_path "$HOME/.local/share/applications/io.sessiondex.desktop.desktop" "desktop entry"
  remove_path "$HOME/.local/share/applications/SessionDex.desktop" "desktop entry"
}

data_paths() {
  case "$(uname -s)" in
    Darwin)
      printf "%s\n" \
        "$HOME/Library/Application Support/io.sessiondex.desktop" \
        "$HOME/Library/Caches/io.sessiondex.desktop" \
        "$HOME/Library/Preferences/io.sessiondex.desktop.plist"
      ;;
    Linux)
      printf "%s\n" \
        "${XDG_DATA_HOME:-$HOME/.local/share}/io.sessiondex.desktop" \
        "${XDG_CACHE_HOME:-$HOME/.cache}/io.sessiondex.desktop" \
        "${XDG_CONFIG_HOME:-$HOME/.config}/io.sessiondex.desktop"
      ;;
  esac
}

print_data_summary() {
  printf "\n%sPreferences and app data%s\n" "$BOLD" "$RESET"
  printf "SessionDex stores its app-owned SQLite metadata as sessiondex.sqlite3 in the app data directory.\n"
  printf "Deleting this data removes custom names, hidden/pinned sessions, settings, and cache.\n"
  printf "Codex and Claude session history will not be touched.\n\n"
  printf "Paths checked:\n"
  data_paths | while IFS= read -r path; do
    printf "  %s\n" "$path"
  done
}

remove_data() {
  data_paths | while IFS= read -r path; do
    remove_path "$path" "preferences/data"
  done
}

case "$(uname -s)" in
  Darwin|Linux) ;;
  *)
    printf "%sUnsupported operating system for uninstall.sh.%s\n" "$RED" "$RESET"
    printf "Use uninstall.ps1 or uninstall.cmd on Windows.\n"
    exit 1
    ;;
esac

printf "%sSessionDex Uninstaller%s\n" "$BOLD" "$RESET"
printf "%s\n\n" "----------------------"
printf "This removes the installed SessionDex app.\n"
printf "Later, this script asks whether to delete SessionDex local data. Press Enter to keep it.\n"

if prompt_yes_no "Remove the SessionDex application now?" "yes"; then
  if ! request_close_running_app; then
    exit 0
  fi

  case "$(uname -s)" in
    Darwin) uninstall_macos_app ;;
    Linux) uninstall_linux_app ;;
  esac
else
  printf "Application removal skipped.\n"
fi

print_data_summary
if confirm_delete_data; then
  if ! request_close_running_app; then
    exit 0
  fi

  remove_data
  printf "%sSessionDex preferences and app data removed.%s\n" "$GREEN" "$RESET"
else
  printf "SessionDex preferences and app data preserved.\n"
fi

printf "\n%sSessionDex uninstall finished.%s\n" "$GREEN" "$RESET"
