use crate::util::home_dir;
use anyhow::Result;
use owo_colors::OwoColorize;
use std::path::PathBuf;
use std::process::Command;

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Browser {
    Safari,
    Chrome,
    Firefox,
    All,
}

pub struct Action {
    pub browser: &'static str,
    pub category: &'static str,
    pub path: PathBuf,
    /// SQL run via the `sqlite3` CLI against `path`. Deliberately scoped to
    /// specific tables — never a whole-file delete — so bookmarks, extensions,
    /// and (critically) saved passwords are structurally impossible to touch
    /// through this path: password tables are never listed here.
    pub sql: &'static str,
    pub process_name: &'static str,
}

fn firefox_profiles() -> Vec<PathBuf> {
    let base = home_dir().join("Library/Application Support/Firefox/Profiles");
    std::fs::read_dir(&base)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default()
}

pub fn actions_for(browser: Browser, history: bool, cookies: bool, autofill: bool) -> Vec<Action> {
    let home = home_dir();
    let mut actions = Vec::new();

    let want = |b: Browser| browser == Browser::All || browser == b;

    if want(Browser::Safari) {
        let base = home.join("Library/Safari");
        if history {
            actions.push(Action {
                browser: "Safari",
                category: "history",
                path: base.join("History.db"),
                sql: "DELETE FROM history_visits; DELETE FROM history_items; VACUUM;",
                process_name: "Safari",
            });
        }
        // Safari's autofill form values and saved-password data share the
        // Keychain/AutoFill store in a way that isn't safely separable via
        // SQL, so it is intentionally not offered here (history/cookies only).
        if cookies {
            actions.push(Action {
                browser: "Safari",
                category: "cookies",
                path: home.join("Library/Cookies/Cookies.binarycookies"),
                sql: "", // binary format, not SQLite — handled as a file delete
                process_name: "Safari",
            });
        }
    }

    if want(Browser::Chrome) {
        let base = home.join("Library/Application Support/Google/Chrome/Default");
        if history {
            actions.push(Action {
                browser: "Chrome",
                category: "history",
                path: base.join("History"),
                sql: "DELETE FROM urls; DELETE FROM visits; DELETE FROM visit_source; VACUUM;",
                process_name: "Google Chrome",
            });
        }
        if cookies {
            actions.push(Action {
                browser: "Chrome",
                category: "cookies",
                path: base.join("Cookies"),
                sql: "DELETE FROM cookies; VACUUM;",
                process_name: "Google Chrome",
            });
        }
        if autofill {
            // "Web Data" holds autofill form/address data. Login Data (saved
            // passwords) is a SEPARATE file and is never referenced here.
            actions.push(Action {
                browser: "Chrome",
                category: "autofill",
                path: base.join("Web Data"),
                sql: "DELETE FROM autofill; DELETE FROM autofill_profiles; VACUUM;",
                process_name: "Google Chrome",
            });
        }
    }

    if want(Browser::Firefox) {
        for profile in firefox_profiles() {
            if history {
                actions.push(Action {
                    browser: "Firefox",
                    category: "history",
                    path: profile.join("places.sqlite"),
                    // Only visit records are cleared; moz_places rows stay so
                    // bookmark targets (which reference moz_places) are untouched.
                    sql: "DELETE FROM moz_historyvisits; VACUUM;",
                    process_name: "firefox",
                });
            }
            if cookies {
                actions.push(Action {
                    browser: "Firefox",
                    category: "cookies",
                    path: profile.join("cookies.sqlite"),
                    sql: "DELETE FROM moz_cookies; VACUUM;",
                    process_name: "firefox",
                });
            }
            if autofill {
                // formhistory.sqlite is form/search autofill only — logins.json
                // and key4.db (saved passwords) are never referenced here.
                actions.push(Action {
                    browser: "Firefox",
                    category: "autofill",
                    path: profile.join("formhistory.sqlite"),
                    sql: "DELETE FROM moz_formhistory; VACUUM;",
                    process_name: "firefox",
                });
            }
        }
    }

    actions.retain(|a| a.path.exists());
    actions
}

pub fn is_running(process_name: &str) -> bool {
    Command::new("pgrep")
        .args(["-x", process_name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn run_action(a: &Action) -> Result<()> {
    if a.sql.is_empty() {
        std::fs::remove_file(&a.path)?;
        return Ok(());
    }
    let status = Command::new("/usr/bin/sqlite3")
        .arg(&a.path)
        .arg(a.sql)
        .status()?;
    if !status.success() {
        anyhow::bail!(
            "sqlite3 exited with {status} for {} — likely needs Full Disk Access for Terminal in \
             System Settings > Privacy & Security, or the browser is still holding a lock on the file",
            a.path.display()
        );
    }
    Ok(())
}

pub fn print_plan(actions: &[Action]) {
    println!("{}", "Privacy cleanup plan".bold());
    for a in actions {
        let running = if is_running(a.process_name) {
            " [RUNNING — close it first]".red().to_string()
        } else {
            String::new()
        };
        println!(
            "  {:<8} {:<9} {}{}",
            a.browser,
            a.category,
            a.path.display(),
            running
        );
    }
    println!(
        "{}",
        "Never touched by this tool: saved passwords, Keychain items, bookmarks, extensions, open tabs/sessions."
            .dimmed()
    );
}
