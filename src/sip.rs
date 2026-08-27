use crate::util::home_dir;
use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Heuristic, read-only triage — NOT a signature-based antivirus engine and
/// no substitute for one. This only surfaces things worth a human look:
/// Gatekeeper/SIP state, non-Apple persistence entries, and crude red flags
/// in their launch commands. Nothing is ever modified, quarantined, or deleted.
pub fn run() {
    println!(
        "{}",
        "SIP/Gatekeeper/persistence check (heuristic — not a virus scanner)".bold()
    );

    check_gatekeeper();
    check_sip();
    check_persistence_dir(&PathBuf::from("/Library/LaunchDaemons"));
    check_persistence_dir(&PathBuf::from("/Library/LaunchAgents"));
    check_persistence_dir(&home_dir().join("Library/LaunchAgents"));
    check_crontab();

    println!(
        "{}",
        "For a real verdict, cross-check anything flagged here with VirusTotal or a dedicated AV product."
            .dimmed()
    );
}

fn check_gatekeeper() {
    if let Ok(out) = Command::new("spctl").arg("--status").output() {
        let s = String::from_utf8_lossy(&out.stdout);
        let status = if s.contains("enabled") {
            "enabled".to_string().green().to_string()
        } else {
            "DISABLED".to_string().red().to_string()
        };
        println!("  Gatekeeper: {}", status);
    }
}

fn check_sip() {
    if let Ok(out) = Command::new("csrutil").arg("status").output() {
        let s = String::from_utf8_lossy(&out.stdout);
        let status = if s.contains("enabled") {
            "enabled".to_string().green().to_string()
        } else {
            "DISABLED".to_string().red().to_string()
        };
        println!("  System Integrity Protection: {}", status);
    }
}

const SUSPICIOUS_SNIPPETS: &[&str] = &[
    "curl",
    "wget",
    "base64 -d",
    "/tmp/",
    "/private/tmp/",
    "osascript -e",
    "nc -",
];

fn check_persistence_dir(dir: &Path) {
    let entries = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };
    for e in entries.filter_map(|e| e.ok()) {
        let path = e.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with("com.apple.") {
            continue;
        }
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let hits: Vec<&str> = SUSPICIOUS_SNIPPETS
            .iter()
            .filter(|s| content.contains(**s))
            .copied()
            .collect();
        if hits.is_empty() {
            println!(
                "  [{}] non-Apple persistence entry: {}",
                "note".yellow(),
                path.display()
            );
        } else {
            println!(
                "  [{}] non-Apple persistence entry with suspicious pattern(s) {:?}: {}",
                "flag".red(),
                hits,
                path.display()
            );
        }
    }
}

fn check_crontab() {
    if let Ok(out) = Command::new("crontab").arg("-l").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            let lines: Vec<&str> = s
                .lines()
                .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
                .collect();
            if !lines.is_empty() {
                println!(
                    "  [{}] {} active user crontab entr(ies) — review with `crontab -l`",
                    "note".yellow(),
                    lines.len()
                );
            }
        }
    }
}
