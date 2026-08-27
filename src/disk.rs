use anyhow::{bail, Result};

/// Read-only filesystem check via Apple's own `diskutil` — safe to run anytime.
pub fn verify(volume: &str) -> Result<()> {
    let status = std::process::Command::new("/usr/sbin/diskutil")
        .args(["verifyVolume", volume])
        .status()?;
    if !status.success() {
        bail!("diskutil verifyVolume exited with {status}");
    }
    Ok(())
}

/// Runs Apple's own repair tool. This does write to the filesystem (it's
/// fixing directory-structure issues diskutil itself finds), so unlike
/// `verify` it goes through the same confirm/--yes gate as `clean`.
/// Note: macOS will refuse to live-repair the sealed system volume; that's
/// diskutil's own restriction, not this tool's.
pub fn repair(volume: &str) -> Result<()> {
    let status = std::process::Command::new("/usr/sbin/diskutil")
        .args(["repairVolume", volume])
        .status()?;
    if !status.success() {
        bail!("diskutil repairVolume exited with {status}");
    }
    Ok(())
}
