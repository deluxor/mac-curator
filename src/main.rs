mod caches;
mod disk;
mod dns;
mod duplicates;
mod sip;
mod memory;
mod privacy;
mod suggest;
mod tempfiles;
mod trash;
mod util;

use anyhow::Result;
use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;
use privacy::Browser;
use std::path::PathBuf;
use util::{fmt_size, is_root};

#[derive(Parser)]
#[command(
    name = "curator",
    version,
    about = "Fast macOS system curator: reclaim RAM, flush DNS, clean caches/temp files, and surface optimizations."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Report memory, cache and temp-file sizes without changing anything.
    Scan,
    /// Force the kernel to reclaim inactive/purgeable memory. Requires sudo.
    Memory,
    /// Flush the DNS resolver cache. Requires sudo.
    Dns,
    /// Empty Trash (your own ~/.Trash plus per-user Trash on mounted volumes).
    Trash {
        #[arg(long)]
        dry_run: bool,
        #[arg(short, long)]
        yes: bool,
    },
    /// Clean caches, stale temp/log files, and empty Trash.
    Clean {
        /// Only report what would be removed; do not delete anything.
        #[arg(long)]
        dry_run: bool,
        /// Skip the confirmation prompt.
        #[arg(short, long)]
        yes: bool,
        /// Also clean caches under /Library/Caches etc. (requires sudo).
        #[arg(long)]
        system: bool,
        /// Leave Trash alone (by default `clean` empties it too).
        #[arg(long)]
        keep_trash: bool,
    },
    /// Print non-destructive optimization suggestions.
    Suggest,
    /// Find duplicate files under a path (size -> partial-hash -> full-hash staged matching).
    Duplicates {
        /// Directory to scan.
        path: PathBuf,
        /// Ignore files smaller than this many bytes.
        #[arg(long, default_value_t = 4096)]
        min_size: u64,
        /// Delete all but the oldest copy in each duplicate group.
        #[arg(long)]
        delete: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(short, long)]
        yes: bool,
    },
    /// Clear browser history/cookies/autofill. Never touches saved passwords, Keychain, or bookmarks.
    Privacy {
        #[arg(long, value_enum, default_value_t = Browser::All)]
        browser: Browser,
        #[arg(long)]
        history: bool,
        #[arg(long)]
        cookies: bool,
        #[arg(long)]
        autofill: bool,
        /// Proceed even if the target browser looks like it's running (risk of the browser overwriting your change or a locked-file failure).
        #[arg(long)]
        force: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(short, long)]
        yes: bool,
    },
    /// Heuristic, read-only SIP/Gatekeeper/persistence triage. Not a substitute for real antivirus.
    SipCheck,
    /// Wrap Apple's diskutil for filesystem verify/repair.
    Disk {
        #[command(subcommand)]
        action: DiskAction,
    },
    /// Run scan + memory + dns + clean + suggest in one pass.
    All {
        #[arg(short, long)]
        yes: bool,
        /// Report everything that would happen (cleanup targets, memory/DNS steps) without changing anything.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum DiskAction {
    /// Read-only filesystem check. Safe to run anytime.
    Verify {
        #[arg(default_value = "/")]
        volume: String,
    },
    /// Runs diskutil's repair (writes to the filesystem to fix what verify found).
    Repair {
        #[arg(default_value = "/")]
        volume: String,
        #[arg(short, long)]
        yes: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Scan => cmd_scan(),
        Commands::Memory => cmd_memory(),
        Commands::Dns => cmd_dns(),
        Commands::Trash { dry_run, yes } => cmd_trash(dry_run, yes),
        Commands::Clean { dry_run, yes, system, keep_trash } => cmd_clean(dry_run, yes, system, keep_trash),
        Commands::Suggest => {
            suggest::run();
            Ok(())
        }
        Commands::Duplicates { path, min_size, delete, dry_run, yes } => {
            cmd_duplicates(path, min_size, delete, dry_run, yes)
        }
        Commands::Privacy { browser, history, cookies, autofill, force, dry_run, yes } => {
            cmd_privacy(browser, history, cookies, autofill, force, dry_run, yes)
        }
        Commands::SipCheck => {
            sip::run();
            Ok(())
        }
        Commands::Disk { action } => cmd_disk(action),
        Commands::All { yes, dry_run } => cmd_all(yes, dry_run),
    }
}

fn cmd_scan() -> Result<()> {
    let mem = memory::snapshot();
    memory::print_snapshot("Memory", &mem);
    println!();
    caches::print_scan_report();
    println!();
    tempfiles::print_scan_report();
    println!();
    trash::print_scan_report();
    Ok(())
}

fn cmd_trash(dry_run: bool, yes: bool) -> Result<()> {
    trash::print_scan_report();
    if dry_run {
        println!("{}", "Dry run — nothing was deleted.".dimmed());
        return Ok(());
    }
    if !yes {
        let proceed = util::confirm("Permanently empty Trash?", false)?;
        if !proceed {
            println!("Aborted.");
            return Ok(());
        }
    }
    let freed = trash::empty_all();
    println!("{} {}", "Trash emptied, freed:".bold(), fmt_size(freed).green());
    Ok(())
}

fn cmd_memory() -> Result<()> {
    let before = memory::snapshot();
    memory::print_snapshot("Before", &before);
    memory::purge_memory()?;
    let after = memory::snapshot();
    memory::print_snapshot("After", &after);
    println!("{}", "Memory purge requested.".green());
    Ok(())
}

fn cmd_dns() -> Result<()> {
    dns::flush_dns()?;
    println!("{}", "DNS cache flushed.".green());
    Ok(())
}

fn cmd_clean(dry_run: bool, yes: bool, system: bool, keep_trash: bool) -> Result<()> {
    if system && !is_root() {
        println!(
            "{}",
            "note: --system requested but not running as root; system-level targets will be skipped."
                .yellow()
        );
    }

    caches::print_scan_report();
    println!();
    tempfiles::print_scan_report();
    println!();
    if !keep_trash {
        trash::print_scan_report();
        println!();
    }

    if dry_run {
        println!("{}", "Dry run — nothing was deleted.".dimmed());
        return Ok(());
    }

    if !yes {
        let prompt = if keep_trash {
            "Proceed with cleanup?"
        } else {
            "Proceed with cleanup (including permanently emptying Trash)?"
        };
        let proceed = util::confirm(prompt, false)?;
        if !proceed {
            println!("Aborted.");
            return Ok(());
        }
    }

    println!("{}", "Cleaning caches...".bold());
    let cache_bytes = caches::clean_all(system);
    println!("{}", "Cleaning temp/log files...".bold());
    let temp_bytes = tempfiles::clean_all(system);
    let trash_bytes = if keep_trash {
        0
    } else {
        println!("{}", "Emptying Trash...".bold());
        trash::empty_all()
    };

    println!(
        "{} {}",
        "Total freed:".bold(),
        fmt_size(cache_bytes + temp_bytes + trash_bytes).green()
    );
    Ok(())
}

fn cmd_all(yes: bool, dry_run: bool) -> Result<()> {
    cmd_scan()?;
    println!();

    if dry_run {
        println!(
            "{}",
            "Dry run — memory purge and DNS flush would run here if not for --dry-run (and if run with sudo).".dimmed()
        );
        println!();
    } else if is_root() {
        cmd_memory()?;
        println!();
        cmd_dns()?;
        println!();
    } else {
        println!(
            "{}",
            "Skipping memory purge and DNS flush (run with sudo to include them).".yellow()
        );
        println!();
    }

    cmd_clean(dry_run, yes, is_root(), false)?;
    println!();
    sip::run();
    println!();
    suggest::run();
    Ok(())
}

fn cmd_duplicates(path: PathBuf, min_size: u64, delete: bool, dry_run: bool, yes: bool) -> Result<()> {
    if !path.is_dir() {
        anyhow::bail!("{} is not a directory", path.display());
    }
    println!("{}", format!("Scanning {} for duplicates...", path.display()).bold());
    let mut groups = duplicates::find_duplicates(&path, min_size);
    if groups.is_empty() {
        println!("No duplicates found.");
        return Ok(());
    }
    groups.sort_by_key(|g| std::cmp::Reverse(g.wasted()));

    let mut total_waste = 0u64;
    for g in &groups {
        total_waste += g.wasted();
        println!("  {} copies x {} (waste {})", g.paths.len(), fmt_size(g.size), fmt_size(g.wasted()).yellow());
        for p in &g.paths {
            println!("    {}", p.display());
        }
    }
    println!("{} {}", "Total reclaimable:".bold(), fmt_size(total_waste).green());

    if !delete {
        println!("{}", "Run with --delete to remove all but the oldest copy in each group.".dimmed());
        return Ok(());
    }
    if dry_run {
        println!("{}", "Dry run — nothing was deleted.".dimmed());
        return Ok(());
    }
    if !yes {
        let proceed = util::confirm(&format!("Delete duplicates and reclaim {}?", fmt_size(total_waste)), false)?;
        if !proceed {
            println!("Aborted.");
            return Ok(());
        }
    }

    let mut freed = 0u64;
    for g in &groups {
        let mut with_mtime: Vec<(&PathBuf, std::time::SystemTime)> = g
            .paths
            .iter()
            .map(|p| {
                let t = std::fs::metadata(p)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                (p, t)
            })
            .collect();
        with_mtime.sort_by_key(|(_, t)| *t);
        for (p, _) in with_mtime.iter().skip(1) {
            if std::fs::remove_file(p).is_ok() {
                freed += g.size;
            }
        }
    }
    println!("{} {}", "Freed:".bold(), fmt_size(freed).green());
    Ok(())
}

fn cmd_privacy(
    browser: Browser,
    history: bool,
    cookies: bool,
    autofill: bool,
    force: bool,
    dry_run: bool,
    yes: bool,
) -> Result<()> {
    if !(history || cookies || autofill) {
        anyhow::bail!("specify at least one of --history, --cookies, --autofill");
    }
    let actions = privacy::actions_for(browser, history, cookies, autofill);
    if actions.is_empty() {
        println!("Nothing to clean for the selected browser(s)/categories.");
        return Ok(());
    }
    privacy::print_plan(&actions);

    if dry_run {
        println!("{}", "Dry run — nothing was changed.".dimmed());
        return Ok(());
    }

    let any_running = actions.iter().any(|a| privacy::is_running(a.process_name));
    if any_running && !force {
        println!(
            "{}",
            "Refusing to proceed: at least one target browser is running. Close it, or pass --force to proceed anyway."
                .red()
        );
        return Ok(());
    }

    if !yes {
        let proceed = util::confirm("Proceed with clearing the listed browser data?", false)?;
        if !proceed {
            println!("Aborted.");
            return Ok(());
        }
    }

    for a in &actions {
        match privacy::run_action(a) {
            Ok(_) => println!("  cleared {} {}", a.browser, a.category),
            Err(e) => println!("  {} {} {}: {e}", "failed".red(), a.browser, a.category),
        }
    }
    Ok(())
}

fn cmd_disk(action: DiskAction) -> Result<()> {
    match action {
        DiskAction::Verify { volume } => {
            println!("{}", format!("Verifying {volume}...").bold());
            disk::verify(&volume)
        }
        DiskAction::Repair { volume, yes } => {
            if !yes {
                let proceed = util::confirm(&format!("Run diskutil repairVolume on {volume}?"), false)?;
                if !proceed {
                    println!("Aborted.");
                    return Ok(());
                }
            }
            println!("{}", format!("Repairing {volume}...").bold());
            disk::repair(&volume)
        }
    }
}
