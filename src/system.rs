//! Lightweight hardware and process information.
//!
//! Everything is read from `/proc` and `/sys` (with a couple of `uname`
//! fields), so the About dialog and the top-bar counters add no dependency
//! and no background process to a modest machine.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::time::Instant;

#[derive(Clone, Debug, Default)]
pub struct HardwareInfo {
    pub cpu: String,
    pub cores: usize,
    pub memory: String,
    pub os: String,
    pub kernel: String,
    pub architecture: String,
    pub device: String,
}

impl HardwareInfo {
    pub fn detect() -> Self {
        let cpu = fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|raw| {
                raw.lines()
                    .find_map(|line| {
                        let (key, value) = line.split_once(':')?;
                        (key.trim() == "model name").then(|| value.trim().to_owned())
                    })
                    .or_else(|| {
                        raw.lines().find_map(|line| {
                            let (key, value) = line.split_once(':')?;
                            (key.trim() == "Processor").then(|| value.trim().to_owned())
                        })
                    })
            })
            .unwrap_or_else(|| "Unknown CPU".to_owned());
        let cores = fs::read_to_string("/proc/cpuinfo")
            .map(|raw| raw.lines().filter(|line| line.starts_with("processor")).count())
            .unwrap_or(0)
            .max(1);
        let memory_kb = fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|raw| {
                raw.lines().find_map(|line| {
                    let rest = line.strip_prefix("MemTotal:")?;
                    rest.split_whitespace().next()?.parse::<u64>().ok()
                })
            })
            .unwrap_or(0);
        let os = fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|raw| {
                raw.lines()
                    .find_map(|line| line.strip_prefix("PRETTY_NAME=").map(str::to_owned))
            })
            .map(|value| value.trim_matches('"').to_owned())
            .unwrap_or_else(|| "Linux".to_owned());
        let kernel = fs::read_to_string("/proc/sys/kernel/osrelease")
            .map(|value| value.trim().to_owned())
            .unwrap_or_default();
        let vendor = read_trim("/sys/class/dmi/id/sys_vendor");
        let product = read_trim("/sys/class/dmi/id/product_name");
        let device = match (vendor.as_deref(), product.as_deref()) {
            (Some(vendor), Some(product)) => format!("{vendor} {product}"),
            (None, Some(product)) => product.to_owned(),
            (Some(vendor), None) => vendor.to_owned(),
            (None, None) => String::new(),
        };
        Self {
            cpu,
            cores,
            memory: human_memory(memory_kb),
            os,
            kernel,
            architecture: std::env::consts::ARCH.to_owned(),
            device,
        }
    }
}

fn read_trim(path: &str) -> Option<String> {
    let value = fs::read_to_string(path).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn human_memory(kb: u64) -> String {
    if kb == 0 {
        return "—".to_owned();
    }
    let gb = kb as f64 / 1_048_576.0;
    if gb >= 1.0 {
        format!("{gb:.1} GB")
    } else {
        format!("{} MB", kb / 1024)
    }
}

/// CPU and memory used by Kodemo and every process it spawned (the
/// WebKitGTK webview, llama-server, preview/terminal helpers), sampled from
/// `/proc`.
pub struct ProcSampler {
    last_ticks: u64,
    last_at: Option<Instant>,
    pub cpu_percent: f32,
    pub memory_mb: f32,
}

impl Default for ProcSampler {
    fn default() -> Self {
        Self {
            last_ticks: 0,
            last_at: None,
            cpu_percent: 0.0,
            memory_mb: 0.0,
        }
    }
}

impl ProcSampler {
    /// Refresh the counters. Cheap enough to call every frame, but the rate
    /// fields are only recomputed about once per second.
    pub fn sample(&mut self) {
        let now = Instant::now();
        let due = self
            .last_at
            .map(|last| now.duration_since(last).as_millis() >= 900)
            .unwrap_or(true);
        if !due {
            return;
        }
        let (ticks, rss) = process_tree_usage(std::process::id());
        if let (Some(ticks), Some(last_at)) = (ticks, self.last_at) {
            let wall = now.duration_since(last_at).as_secs_f32().max(0.001);
            let tick_rate = clock_ticks_per_second() as f32;
            let delta = ticks.saturating_sub(self.last_ticks) as f32 / tick_rate;
            // Share of a single core, matching what `top` prints for one
            // process. A busy editor can legitimately exceed 100%.
            self.cpu_percent = (delta / wall * 100.0).clamp(0.0, 999.0);
        }
        if let Some(ticks) = ticks {
            self.last_ticks = ticks;
        }
        if let Some(rss) = rss {
            self.memory_mb = rss as f32 / 1024.0;
        }
        self.last_at = Some(now);
    }
}

fn clock_ticks_per_second() -> i64 {
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if ticks > 0 {
        ticks
    } else {
        100
    }
}

/// Sum utime+stime (clock ticks) and resident memory (kB) over `root` and all
/// of its descendants. This is what the task manager would call the app's
/// whole tree: the editor, the WebKit webview and the local Qwen server.
fn process_tree_usage(root: u32) -> (Option<u64>, Option<u64>) {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut usage: HashMap<u32, (u64, u64)> = HashMap::new();
    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<u32>().ok())
            else {
                continue;
            };
            let Some((ppid, ticks, rss)) = proc_pid_usage(pid) else {
                continue;
            };
            children.entry(ppid).or_default().push(pid);
            usage.insert(pid, (ticks, rss));
        }
    }
    let mut stack = vec![root];
    let mut seen = HashSet::new();
    let mut ticks = 0u64;
    let mut rss = 0u64;
    let mut found = false;
    while let Some(pid) = stack.pop() {
        if !seen.insert(pid) {
            continue;
        }
        if let Some((cpu, memory)) = usage.get(&pid) {
            ticks = ticks.saturating_add(*cpu);
            rss = rss.saturating_add(*memory);
            found = true;
        }
        if let Some(kids) = children.get(&pid) {
            stack.extend(kids.iter().copied());
        }
    }
    (found.then_some(ticks), found.then_some(rss))
}

/// `(ppid, utime+stime ticks, VmRSS kB)` for one pid, read from /proc.
fn proc_pid_usage(pid: u32) -> Option<(u32, u64, u64)> {
    let raw = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The command name is wrapped in parentheses and may contain spaces, so
    // split after the final ')'.
    let tail = &raw[raw.rfind(')')? + 1..];
    let fields: Vec<&str> = tail.split_whitespace().collect();
    // State is field 3 and the parent pid is field 4, so after the command
    // name they are at indices 0 and 1. Fields 14 and 15 are utime and stime,
    // at indices 11 and 12.
    let ppid = fields.get(1)?.parse::<u32>().ok()?;
    let utime = fields.get(11)?.parse::<u64>().ok()?;
    let stime = fields.get(12)?.parse::<u64>().ok()?;
    let rss = fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()
        .and_then(|status| {
            status.lines().find_map(|line| {
                let rest = line.strip_prefix("VmRSS:")?;
                rest.split_whitespace().next()?.parse::<u64>().ok()
            })
        })
        .unwrap_or(0);
    Some((ppid, utime + stime, rss))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_hardware_without_panicking() {
        let info = HardwareInfo::detect();
        assert!(info.cores >= 1);
        assert!(!info.os.is_empty());
    }

    #[test]
    fn process_tree_includes_this_process() {
        let (ticks, rss) = process_tree_usage(std::process::id());
        assert!(ticks.is_some());
        assert!(rss.unwrap_or(0) > 0);
    }

    #[test]
    fn memory_is_human_readable() {
        assert_eq!(human_memory(0), "—");
        assert_eq!(human_memory(512 * 1024), "512 MB");
        assert_eq!(human_memory(8 * 1024 * 1024), "8.0 GB");
    }
}
