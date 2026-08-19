#[cfg(target_os = "linux")]
use std::fs;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub const FULL_EFFECTS_PROFILE: &str = "full";
pub const BALANCED_PROFILE: &str = "balanced";
pub const EFFICIENCY_PROFILE: &str = "efficiency";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphicsCapability {
    Hardware,
    Software,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
struct SystemCapabilities {
    logical_cpus: usize,
    total_memory_bytes: Option<u64>,
    graphics: GraphicsCapability,
}

pub fn recommended_rendering_profile() -> &'static str {
    recommended_profile(SystemCapabilities {
        logical_cpus: thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1),
        total_memory_bytes: total_memory_bytes(),
        graphics: graphics_capability(),
    })
}

pub fn is_rendering_profile(value: &str) -> bool {
    matches!(
        value,
        FULL_EFFECTS_PROFILE | BALANCED_PROFILE | EFFICIENCY_PROFILE
    )
}

fn recommended_profile(capabilities: SystemCapabilities) -> &'static str {
    const FOUR_GIB: u64 = 4 * 1024 * 1024 * 1024;
    const SIX_GIB: u64 = 6 * 1024 * 1024 * 1024;

    if capabilities.graphics == GraphicsCapability::Software
        || capabilities.logical_cpus <= 2
        || capabilities
            .total_memory_bytes
            .is_some_and(|memory| memory < FOUR_GIB)
    {
        return EFFICIENCY_PROFILE;
    }

    if capabilities.graphics == GraphicsCapability::Unknown
        || capabilities.logical_cpus <= 4
        || capabilities
            .total_memory_bytes
            .is_some_and(|memory| memory < SIX_GIB)
    {
        return BALANCED_PROFILE;
    }

    FULL_EFFECTS_PROFILE
}

#[cfg(target_os = "linux")]
fn total_memory_bytes() -> Option<u64> {
    let meminfo = fs::read_to_string("/proc/meminfo").ok()?;
    let kilobytes = meminfo.lines().find_map(|line| {
        let value = line.strip_prefix("MemTotal:")?.trim();
        value.split_whitespace().next()?.parse::<u64>().ok()
    })?;

    kilobytes.checked_mul(1024)
}

#[cfg(target_os = "macos")]
fn total_memory_bytes() -> Option<u64> {
    command_stdout("sysctl", &["-n", "hw.memsize"], Duration::from_secs(2))?
        .trim()
        .parse()
        .ok()
}

#[cfg(target_os = "windows")]
fn total_memory_bytes() -> Option<u64> {
    command_stdout(
        "powershell.exe",
        &[
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory",
        ],
        Duration::from_secs(3),
    )?
    .trim()
    .parse()
    .ok()
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn total_memory_bytes() -> Option<u64> {
    None
}

#[cfg(target_os = "linux")]
fn graphics_capability() -> GraphicsCapability {
    if let Some(details) = command_stdout("glxinfo", &["-B"], Duration::from_secs(2)) {
        let details = details.to_lowercase();

        if details.contains("accelerated: no")
            || details.contains("llvmpipe")
            || details.contains("softpipe")
            || details.contains("software rasterizer")
        {
            return GraphicsCapability::Software;
        }

        if details.contains("accelerated: yes") {
            return GraphicsCapability::Hardware;
        }
    }

    match fs::read_dir("/dev/dri") {
        Ok(entries) => {
            if entries
                .flatten()
                .any(|entry| entry.file_name().to_string_lossy().starts_with("renderD"))
            {
                GraphicsCapability::Hardware
            } else {
                GraphicsCapability::Unknown
            }
        }
        Err(_) => GraphicsCapability::Software,
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn graphics_capability() -> GraphicsCapability {
    GraphicsCapability::Hardware
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn graphics_capability() -> GraphicsCapability {
    GraphicsCapability::Unknown
}

fn command_stdout(program: &str, args: &[&str], timeout: Duration) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started_at = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().ok()?;
                return status
                    .success()
                    .then(|| String::from_utf8_lossy(&output.stdout).to_string());
            }
            Ok(None) if started_at.elapsed() < timeout => {
                thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn software_rendering_uses_efficiency() {
        assert_eq!(
            recommended_profile(SystemCapabilities {
                logical_cpus: 16,
                total_memory_bytes: Some(32 * 1024 * 1024 * 1024),
                graphics: GraphicsCapability::Software,
            }),
            EFFICIENCY_PROFILE
        );
    }

    #[test]
    fn constrained_cpu_uses_efficiency() {
        assert_eq!(
            recommended_profile(SystemCapabilities {
                logical_cpus: 2,
                total_memory_bytes: Some(8 * 1024 * 1024 * 1024),
                graphics: GraphicsCapability::Hardware,
            }),
            EFFICIENCY_PROFILE
        );
    }

    #[test]
    fn midrange_system_uses_balanced() {
        assert_eq!(
            recommended_profile(SystemCapabilities {
                logical_cpus: 4,
                total_memory_bytes: Some(8 * 1024 * 1024 * 1024),
                graphics: GraphicsCapability::Hardware,
            }),
            BALANCED_PROFILE
        );
    }

    #[test]
    fn capable_system_uses_full_effects() {
        assert_eq!(
            recommended_profile(SystemCapabilities {
                logical_cpus: 8,
                total_memory_bytes: Some(8 * 1024 * 1024 * 1024),
                graphics: GraphicsCapability::Hardware,
            }),
            FULL_EFFECTS_PROFILE
        );
    }
}
