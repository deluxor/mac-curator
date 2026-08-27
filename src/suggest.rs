use crate::util::{fmt_size, home_dir, scan_files, total_size};
use owo_colors::OwoColorize;
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

pub fn run() {
    println!("{}", "Optimization suggestions".bold());
    let mut any = false;

    any |= check_large_dir("Homebrew download cache", &brew_cache_dir(), 512 * 1024 * 1024, "brew cleanup");
    any |= check_large_dir(
        "iOS/iPadOS device backups",
        &home_dir().join("Library/Application Support/MobileSync/Backup"),
        1024 * 1024 * 1024,
        "delete via Finder > Manage Backups, or `tmutil` if unneeded",
    );
    any |= check_large_dir(
        "Xcode DerivedData",
        &home_dir().join("Library/Developer/Xcode/DerivedData"),
        1024 * 1024 * 1024,
        "curator clean --caches (safe, Xcode regenerates it)",
    );
    any |= check_large_dir(
        "CoreSimulator devices/caches",
        &home_dir().join("Library/Developer/CoreSimulator"),
        1024 * 1024 * 1024,
        "xcrun simctl delete unavailable",
    );
    any |= check_large_dir("Trash", &home_dir().join(".Trash"), 512 * 1024 * 1024, "empty Trash");
    any |= check_docker();
    any |= check_login_items();
    any |= check_time_machine_local_snapshots();
    any |= check_spotlight_indexing();
    any |= check_large_old_downloads();
    any |= check_orphaned_app_support();
    any |= check_xcode_archives();

    if !any {
        println!("  nothing notable found — system looks tidy.");
    }
}

fn brew_cache_dir() -> std::path::PathBuf {
    if let Ok(out) = Command::new("brew").arg("--cache").output() {
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                return std::path::PathBuf::from(p);
            }
        }
    }
    home_dir().join("Library/Caches/Homebrew")
}

fn check_large_dir(label: &str, path: &Path, threshold: u64, action: &str) -> bool {
    if !path.exists() {
        return false;
    }
    let size = total_size(path);
    if size >= threshold {
        println!("  [{}] {} — suggested: {}", fmt_size(size).yellow(), label, action);
        true
    } else {
        false
    }
}

fn check_docker() -> bool {
    let has_docker = Command::new("which").arg("docker").output().map(|o| o.status.success()).unwrap_or(false);
    if !has_docker {
        return false;
    }
    println!(
        "  [{}] Docker detected — run `docker system prune -af --volumes` to reclaim unused images/volumes",
        "check".yellow()
    );
    true
}

fn check_login_items() -> bool {
    let out = Command::new("osascript")
        .args(["-e", "tell application \"System Events\" to get the name of every login item"])
        .output();
    if let Ok(o) = out {
        if o.status.success() {
            let items = String::from_utf8_lossy(&o.stdout);
            let count = items.split(',').filter(|s| !s.trim().is_empty()).count();
            if count > 8 {
                println!(
                    "  [{}] {} login items enabled — review System Settings > General > Login Items for startup slowdown",
                    "check".yellow(),
                    count
                );
                return true;
            }
        }
    }
    false
}

fn check_time_machine_local_snapshots() -> bool {
    let out = Command::new("tmutil").arg("listlocalsnapshots").arg("/").output();
    if let Ok(o) = out {
        if o.status.success() {
            let n = String::from_utf8_lossy(&o.stdout).lines().filter(|l| l.contains("com.apple.TimeMachine")).count();
            if n > 0 {
                println!(
                    "  [{}] {} local Time Machine snapshot(s) held on disk — `tmutil thinlocalsnapshots / 999999999999 4` to reclaim",
                    "check".yellow(),
                    n
                );
                return true;
            }
        }
    }
    false
}

fn check_large_old_downloads() -> bool {
    let dir = home_dir().join("Downloads");
    if !dir.exists() {
        return false;
    }
    let now = SystemTime::now();
    let cutoff_secs = 90 * 24 * 3600;
    let big_old: Vec<(std::path::PathBuf, u64)> = scan_files(&dir)
        .into_iter()
        .filter(|(_, size)| *size >= 200 * 1024 * 1024)
        .filter(|(path, _)| {
            std::fs::metadata(path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|mtime| now.duration_since(mtime).ok())
                .map(|age| age.as_secs() >= cutoff_secs)
                .unwrap_or(false)
        })
        .collect();
    if big_old.is_empty() {
        return false;
    }
    let total: u64 = big_old.iter().map(|(_, s)| s).sum();
    println!(
        "  [{}] {} large file(s) in Downloads untouched for 90+ days, {} total — review manually, not auto-deleted",
        "check".yellow(),
        big_old.len(),
        fmt_size(total)
    );
    true
}

/// Heuristic only: folders under Application Support / Preferences whose name
/// doesn't loosely match any installed app in /Applications. Never deleted
/// automatically — installers, helper tools, and shared frameworks legitimately
/// leave folders that don't match an app name 1:1.
fn check_orphaned_app_support() -> bool {
    let apps_dir = Path::new("/Applications");
    let installed: Vec<String> = std::fs::read_dir(apps_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().to_str().map(|s| s.to_lowercase().replace(".app", "")))
                .collect()
        })
        .unwrap_or_default();
    if installed.is_empty() {
        return false;
    }

    let support_dir = home_dir().join("Library/Application Support");
    let candidates: Vec<(String, u64)> = std::fs::read_dir(&support_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter_map(|e| {
                    let name = e.file_name().to_str()?.to_string();
                    let lower = name.to_lowercase();
                    // First-party system components (com.apple.*) are not
                    // 1:1 with /Applications entries — never flag them.
                    if lower.starts_with("com.apple.") {
                        return None;
                    }
                    let matches_installed = installed.iter().any(|app| lower.contains(app) || app.contains(&lower));
                    if matches_installed {
                        return None;
                    }
                    let size = total_size(&e.path());
                    if size < 200 * 1024 * 1024 {
                        return None;
                    }
                    Some((name, size))
                })
                .collect()
        })
        .unwrap_or_default();

    if candidates.is_empty() {
        return false;
    }
    for (name, size) in &candidates {
        println!(
            "  [{}] ~/Library/Application Support/{} ({}) has no matching app in /Applications — possible uninstall leftover, review before deleting",
            "check".yellow(),
            name,
            fmt_size(*size)
        );
    }
    true
}

fn check_xcode_archives() -> bool {
    check_large_dir(
        "Xcode Archives (App Store submission builds)",
        &home_dir().join("Library/Developer/Xcode/Archives"),
        1024 * 1024 * 1024,
        "review in Xcode Organizer before deleting — needed to re-submit/symbolicate crashes for past releases",
    )
}

fn check_spotlight_indexing() -> bool {
    let out = Command::new("mdutil").args(["-s", "/"]).output();
    if let Ok(o) = out {
        let s = String::from_utf8_lossy(&o.stdout);
        if s.contains("Indexing enabled") == false && s.contains("disabled") {
            println!("  [{}] Spotlight indexing is disabled on / — searches will be slow", "note".yellow());
            return true;
        }
    }
    false
}
