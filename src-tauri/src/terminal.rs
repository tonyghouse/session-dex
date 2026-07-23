use crate::models::AppSettings;
use crate::providers::ResumeCommand;
#[cfg(target_os = "windows")]
use std::path::Path;
use std::process::{Command, Stdio};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

pub fn command_available(program: &str) -> bool {
    let program = program.trim();

    if program.is_empty() {
        return false;
    }

    static CACHE: OnceLock<Mutex<HashMap<String, (Instant, bool)>>> = OnceLock::new();
    const CACHE_DURATION: Duration = Duration::from_secs(30);

    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut cache) = cache.lock() else {
        return command_available_uncached(program);
    };

    if let Some((checked_at, available)) = cache.get(program) {
        if checked_at.elapsed() < CACHE_DURATION {
            return *available;
        }
    }

    let available = command_available_uncached(program);
    cache.insert(program.to_string(), (Instant::now(), available));
    available
}

#[cfg(target_os = "windows")]
fn command_available_uncached(program: &str) -> bool {
    if program.contains('\\') || program.contains('/') {
        return Path::new(program).is_file();
    }

    Command::new("where.exe")
        .arg(program)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(not(target_os = "windows"))]
fn command_available_uncached(program: &str) -> bool {
    let check = format!("command -v {}", shell_escape(program));
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

    Command::new(&shell)
        .args(["-lc", &check])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .or_else(|_| {
            Command::new("/bin/sh")
                .args(["-lc", &check])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
        })
        .is_ok_and(|status| status.success())
}

pub fn launch(settings: &AppSettings, resume_command: &ResumeCommand) -> Result<(), String> {
    let command = shell_command(resume_command);
    let terminal = settings.terminal_executable.as_deref().map(str::trim);
    let terminal = terminal.filter(|value| !value.is_empty());

    launch_platform_terminal(terminal, &command)
}

pub fn shell_command(command: &ResumeCommand) -> String {
    platform_shell_command(command)
}

#[cfg(not(target_os = "windows"))]
fn platform_shell_command(command: &ResumeCommand) -> String {
    let resume = shell_join(command);

    match command.working_directory.as_deref().map(str::trim) {
        Some(working_directory) if !working_directory.is_empty() => {
            format!("cd {} && {}", shell_escape(working_directory), resume)
        }
        _ => resume,
    }
}

#[cfg(target_os = "windows")]
fn platform_shell_command(command: &ResumeCommand) -> String {
    let resume = powershell_join(command);

    match command.working_directory.as_deref().map(str::trim) {
        Some(working_directory) if !working_directory.is_empty() => {
            format!(
                "Set-Location -LiteralPath {}; if ($?) {{ {} }}",
                powershell_escape(working_directory),
                resume
            )
        }
        _ => resume,
    }
}

#[cfg(not(target_os = "windows"))]
fn shell_join(command: &ResumeCommand) -> String {
    let mut parts = Vec::with_capacity(command.args.len() + 1);
    parts.push(shell_escape(command.program));
    parts.extend(command.args.iter().map(|arg| shell_escape(arg)));
    parts.join(" ")
}

#[cfg(target_os = "windows")]
fn powershell_join(command: &ResumeCommand) -> String {
    let mut parts = Vec::with_capacity(command.args.len() + 2);
    parts.push("&".to_string());
    parts.push(powershell_escape(command.program));
    parts.extend(command.args.iter().map(|arg| powershell_escape(arg)));
    parts.join(" ")
}

#[cfg(not(target_os = "windows"))]
fn shell_escape(value: &str) -> String {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_@%+=:,./-".contains(&byte))
    {
        return value.to_string();
    }

    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(target_os = "windows")]
fn powershell_escape(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(target_os = "macos")]
fn launch_platform_terminal(terminal: Option<&str>, command: &str) -> Result<(), String> {
    let terminal = terminal.unwrap_or("Terminal");
    let normalized = terminal.to_lowercase();

    if normalized == "terminal" || normalized.ends_with("terminal.app") {
        let do_script = format!(
            "tell application \"Terminal\" to do script {}",
            applescript_string(command)
        );

        Command::new("osascript")
            .arg("-e")
            .arg(do_script)
            .arg("-e")
            .arg("tell application \"Terminal\" to activate")
            .spawn()
            .map_err(|err| err.to_string())?;

        return Ok(());
    }

    if normalized == "iterm"
        || normalized == "iterm2"
        || normalized.ends_with("iterm.app")
        || normalized.ends_with("iterm2.app")
    {
        let script = format!(
            "tell application \"iTerm\" to create window with default profile command {}",
            applescript_string(command)
        );

        Command::new("osascript")
            .arg("-e")
            .arg(script)
            .arg("-e")
            .arg("tell application \"iTerm\" to activate")
            .spawn()
            .map_err(|err| err.to_string())?;

        return Ok(());
    }

    Err("macOS terminal launching currently supports Terminal or iTerm. Leave the setting blank to use Terminal.".to_string())
}

#[cfg(target_os = "macos")]
fn applescript_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(target_os = "linux")]
fn launch_platform_terminal(terminal: Option<&str>, command: &str) -> Result<(), String> {
    let terminal = match terminal {
        Some(value) => value.to_string(),
        None => detect_linux_terminal().ok_or_else(|| {
            "No supported terminal emulator found. Set a terminal executable in Settings."
                .to_string()
        })?,
    };

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let executable_name = terminal
        .rsplit('/')
        .next()
        .unwrap_or(&terminal)
        .to_ascii_lowercase();

    let mut process = Command::new(&terminal);

    match executable_name.as_str() {
        "gnome-terminal" | "kgx" => {
            process.args(["--", &shell, "-lc", command]);
        }
        "xfce4-terminal" | "mate-terminal" | "tilix" => {
            let shell_command = format!("{} -lc {}", shell_escape(&shell), shell_escape(command));
            process.args(["-e", &shell_command]);
        }
        _ => {
            process.args(["-e", &shell, "-lc", command]);
        }
    }

    process.spawn().map_err(|err| err.to_string())?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn detect_linux_terminal() -> Option<String> {
    let candidates = [
        "x-terminal-emulator",
        "gnome-terminal",
        "kgx",
        "konsole",
        "xfce4-terminal",
        "mate-terminal",
        "tilix",
        "alacritty",
        "kitty",
        "wezterm",
        "xterm",
    ];

    candidates
        .iter()
        .find(|candidate| command_available(candidate))
        .map(|candidate| candidate.to_string())
}

#[cfg(target_os = "windows")]
fn launch_platform_terminal(terminal: Option<&str>, command: &str) -> Result<(), String> {
    let terminal = match terminal {
        Some(value) => value.to_string(),
        None => detect_windows_terminal()
            .ok_or_else(|| "No supported Windows terminal found.".to_string())?,
    };
    let normalized = terminal.trim().to_ascii_lowercase();
    let executable = if normalized == "windows terminal" || normalized == "windows-terminal" {
        "wt.exe"
    } else {
        terminal.as_str()
    };
    let executable_name = windows_executable_name(executable);

    let mut process = Command::new(executable);

    match executable_name.as_str() {
        "wt" | "wt.exe" => {
            process.args(["new-tab", "powershell.exe", "-NoExit", "-Command", command]);
        }
        "pwsh" | "pwsh.exe" | "powershell" | "powershell.exe" => {
            process.args(["-NoExit", "-Command", command]);
        }
        "cmd" | "cmd.exe" => {
            process.args(["/K", "powershell.exe", "-NoExit", "-Command", command]);
        }
        _ => {
            return Err("Windows terminal launching currently supports Windows Terminal, PowerShell, or Command Prompt. Leave the setting blank to auto-detect.".to_string());
        }
    }

    process.spawn().map_err(|err| err.to_string())?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn detect_windows_terminal() -> Option<String> {
    let candidates = ["wt.exe", "pwsh.exe", "powershell.exe", "cmd.exe"];

    candidates
        .iter()
        .find(|candidate| command_available(candidate))
        .map(|candidate| candidate.to_string())
}

#[cfg(target_os = "windows")]
fn windows_executable_name(value: &str) -> String {
    Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(value)
        .to_ascii_lowercase()
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn launch_platform_terminal(_terminal: Option<&str>, _command: &str) -> Result<(), String> {
    Err(
        "SessionDex currently supports terminal launching on macOS, Linux, and Windows."
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn shell_command_prefixes_working_directory() {
        let command = ResumeCommand {
            program: "codex",
            args: vec!["resume".to_string(), "abc-123".to_string()],
            working_directory: Some("/Users/test/My Project".to_string()),
        };

        assert_eq!(
            shell_command(&command),
            "cd '/Users/test/My Project' && codex resume abc-123"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn shell_command_prefixes_working_directory() {
        let command = ResumeCommand {
            program: "codex",
            args: vec!["resume".to_string(), "abc-123".to_string()],
            working_directory: Some("C:\\Users\\test\\My Project".to_string()),
        };

        assert_eq!(
            shell_command(&command),
            "Set-Location -LiteralPath 'C:\\Users\\test\\My Project'; if ($?) { & 'codex' 'resume' 'abc-123' }"
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn shell_command_omits_cd_without_working_directory() {
        let command = ResumeCommand {
            program: "claude",
            args: vec!["--resume".to_string(), "abc-123".to_string()],
            working_directory: None,
        };

        assert_eq!(shell_command(&command), "claude --resume abc-123");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn shell_command_omits_cd_without_working_directory() {
        let command = ResumeCommand {
            program: "claude",
            args: vec!["--resume".to_string(), "abc-123".to_string()],
            working_directory: None,
        };

        assert_eq!(shell_command(&command), "& 'claude' '--resume' 'abc-123'");
    }
}
