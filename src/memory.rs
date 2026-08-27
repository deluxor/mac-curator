use crate::util::{fmt_size, is_root};
use anyhow::{bail, Result};
use owo_colors::OwoColorize;
use sysinfo::System;

pub struct MemSnapshot {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub swap_used: u64,
    pub swap_total: u64,
}

pub fn snapshot() -> MemSnapshot {
    let mut sys = System::new();
    sys.refresh_memory();
    MemSnapshot {
        total: sys.total_memory(),
        used: sys.used_memory(),
        available: sys.available_memory(),
        swap_used: sys.used_swap(),
        swap_total: sys.total_swap(),
    }
}

pub fn print_snapshot(label: &str, s: &MemSnapshot) {
    println!("{}", label.bold());
    println!(
        "  RAM   {} used / {} total  ({} available)",
        fmt_size(s.used),
        fmt_size(s.total),
        fmt_size(s.available)
    );
    if s.swap_total > 0 {
        println!(
            "  Swap  {} used / {} total",
            fmt_size(s.swap_used),
            fmt_size(s.swap_total)
        );
    }
}

/// Ask the kernel to reclaim inactive/purgeable memory via the `purge` binary.
/// This forces the VM system to evict file-backed and purgeable pages that
/// are safe to drop. Requires root.
pub fn purge_memory() -> Result<()> {
    if !is_root() {
        bail!("purging memory requires root (re-run with sudo)");
    }
    let status = std::process::Command::new("/usr/sbin/purge").status()?;
    if !status.success() {
        bail!("`purge` exited with status {status}");
    }
    Ok(())
}
