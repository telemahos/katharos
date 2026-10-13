//! Πλήρης απεγκατάσταση εφαρμογών μαζί με τα κατάλοιπά τους στη Library.

use crate::junk::Item;
use crate::util::*;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicU64;

#[derive(Clone, Debug)]
pub struct AppInfo {
    pub name: String,
    pub id: String,
    pub version: String,
    pub path: PathBuf,
    pub size: u64,
}

fn read_info(app: &Path) -> Option<(String, String)> {
    let v = plist::Value::from_file(app.join("Contents/Info.plist")).ok()?;
    let d = v.as_dictionary()?;
    let id = d.get("CFBundleIdentifier")?.as_string()?.to_string();
    let ver = d
        .get("CFBundleShortVersionString")
        .and_then(|x| x.as_string())
        .unwrap_or("")
        .to_string();
    Some((id, ver))
}

pub fn list(c: &AtomicU64) -> Vec<AppInfo> {
    let roots = [PathBuf::from("/Applications"), home().join("Applications")];
    let mut bundles = Vec::new();
    for r in &roots {
        for ch in children(r) {
            if ext_lower(&ch) == "app" {
                bundles.push(ch);
            } else if ch.is_dir() {
                // π.χ. /Applications/Adobe Photoshop/Adobe Photoshop.app
                bundles.extend(children(&ch).into_iter().filter(|g| ext_lower(g) == "app"));
            }
        }
    }
    let mut apps: Vec<AppInfo> = bundles
        .par_iter()
        .filter_map(|p| {
            let (id, version) = read_info(p)?;
            if id.starts_with("com.apple.") {
                return None; // εφαρμογές συστήματος: προστατεύονται από το macOS
            }
            let name = p.file_stem()?.to_string_lossy().to_string();
            Some(AppInfo { name, id, version, path: p.clone(), size: path_size(p, c) })
        })
        .collect();
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

/// Φάκελοι της ~/Library όπου μένουν κατάλοιπα. Το bool = επιτρέπεται ταίριασμα με το όνομα της εφαρμογής.
const LEFTOVER_DIRS: &[(&str, bool)] = &[
    ("Application Support", true),
    ("Caches", true),
    ("Logs", true),
    ("Preferences", false),
    ("Preferences/ByHost", false),
    ("Containers", false),
    ("Group Containers", false),
    ("Saved Application State", false),
    ("HTTPStorages", false),
    ("WebKit", false),
    ("Cookies", false),
    ("LaunchAgents", false),
    ("Application Scripts", false),
];

pub fn matches(entry: &str, id: &str, name: &str, allow_name: bool) -> bool {
    let n = entry.to_lowercase();
    let id = id.to_lowercase();
    let name = name.to_lowercase();
    n == id
        || n.starts_with(&format!("{}.", id))
        || n.ends_with(&format!(".{}", id))
        || (allow_name && name.chars().count() >= 4 && n == name)
}

pub fn leftovers(app: &AppInfo, c: &AtomicU64) -> Vec<Item> {
    let lib = home().join("Library");
    let mut found: Vec<PathBuf> = Vec::new();
    for (d, allow_name) in LEFTOVER_DIRS {
        for ch in children(&lib.join(d)) {
            if matches(&file_name(&ch), &app.id, &app.name, *allow_name) {
                found.push(ch);
            }
        }
    }
    let mut items: Vec<Item> = found
        .par_iter()
        .map(|p| Item { path: p.clone(), size: path_size(p, c), selected: true })
        .collect();
    items.sort_by(|a, b| b.size.cmp(&a.size));
    items
}

/// Κλείνει την εφαρμογή και μετακινεί εφαρμογή + κατάλοιπα στον Κάδο.
pub fn uninstall(app: &AppInfo, items: &[Item]) -> (Vec<PathBuf>, u64, Vec<String>) {
    let _ = Command::new("osascript")
        .args(["-e", &format!("quit app id \"{}\"", app.id.replace('"', ""))])
        .output();
    std::thread::sleep(std::time::Duration::from_millis(800));
    let mut paths = vec![app.path.clone()];
    paths.extend(items.iter().filter(|i| i.selected).map(|i| i.path.clone()));
    let (ok, errs) = trash_many(&paths);
    let mut size = if ok.contains(&app.path) { app.size } else { 0 };
    size += items.iter().filter(|i| ok.contains(&i.path)).map(|i| i.size).sum::<u64>();
    (ok, size, errs)
}
