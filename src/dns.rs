use crate::util::is_root;
use anyhow::{bail, Result};

/// Flush the macOS DNS resolver cache and restart mDNSResponder.
/// Requires root (the same as running `sudo dscacheutil -flushcache`).
pub fn flush_dns() -> Result<()> {
    if !is_root() {
        bail!("flushing DNS requires root (re-run with sudo)");
    }
    let s1 = std::process::Command::new("/usr/bin/dscacheutil")
        .arg("-flushcache")
        .status()?;
    let s2 = std::process::Command::new("/usr/bin/killall")
        .args(["-HUP", "mDNSResponder"])
        .status()?;
    if !s1.success() || !s2.success() {
        bail!("DNS flush commands did not all succeed");
    }
    Ok(())
}
