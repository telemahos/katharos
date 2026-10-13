//! Έξυπνη σάρωση: cache, logs, Xcode, συνημμένα Mail, εγκαταστάτες, Κάδος κ.λπ.

use crate::i18n::K;
use crate::util::*;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Action {
    /// Οριστική διαγραφή περιεχομένων (ασφαλές: ξαναδημιουργούνται).
    Purge,
    /// Μετακίνηση στον Κάδο (αρχεία χρήστη, ανακτήσιμα).
    Trash,
    /// Άδειασμα Κάδου.
    EmptyTrash,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub path: PathBuf,
    pub size: u64,
    pub selected: bool,
}

#[derive(Clone, Debug)]
pub struct Category {
    pub name: K,
    pub desc: K,
    pub action: Action,
    pub items: Vec<Item>,
}

impl Category {
    pub fn total(&self) -> u64 {
        self.items.iter().map(|i| i.size).sum()
    }
    pub fn selected_size(&self) -> u64 {
        self.items.iter().filter(|i| i.selected).map(|i| i.size).sum()
    }
    pub fn selected_count(&self) -> usize {
        self.items.iter().filter(|i| i.selected).count()
    }
    pub fn set_all(&mut self, v: bool) {
        self.items.iter_mut().for_each(|i| i.selected = v);
    }
}

enum Src {
    /// Κάθε παιδί του φακέλου είναι ξεχωριστό στοιχείο.
    Children(PathBuf),
    /// Ο ίδιος ο φάκελος είναι ένα στοιχείο.
    Whole(PathBuf),
    /// Παιδιά του φακέλου που περνούν το φίλτρο.
    Filtered(PathBuf, fn(&Path) -> bool),
}

struct Spec {
    name: K,
    desc: K,
    action: Action,
    on: bool,
    src: Vec<Src>,
}

/// Cache που ανήκουν σε υπηρεσίες iCloud/συστήματος — δεν τα πειράζουμε.
const CACHE_SKIP: &[&str] = &[
    "com.apple.bird",
    "CloudKit",
    "com.apple.containermanagerd",
    "com.apple.HomeKit",
    "com.apple.homed",
    "FamilyCircle",
    "com.apple.akd",
    "com.apple.ap.adprivacyd",
    "com.apple.nsurlsessiond",
];

const NOISE: &[&str] = &[".DS_Store", ".localized"];

fn is_installer(p: &Path) -> bool {
    matches!(ext_lower(p).as_str(), "dmg" | "pkg" | "mpkg" | "xip")
}

fn specs() -> Vec<Spec> {
    let h = home();
    let l = h.join("Library");
    let dev = l.join("Developer");
    vec![
        Spec {
            name: K::CatCache,
            desc: K::CatCacheDesc,
            action: Action::Purge,
            on: true,
            src: vec![Src::Children(l.join("Caches"))],
        },
        Spec {
            name: K::CatLogs,
            desc: K::CatLogsDesc,
            action: Action::Purge,
            on: true,
            src: vec![Src::Children(l.join("Logs"))],
        },
        Spec {
            name: K::CatState,
            desc: K::CatStateDesc,
            action: Action::Purge,
            on: true,
            src: vec![Src::Children(l.join("Saved Application State"))],
        },
        Spec {
            name: K::CatXcode,
            desc: K::CatXcodeDesc,
            action: Action::Purge,
            on: true,
            src: vec![
                Src::Children(dev.join("Xcode/DerivedData")),
                Src::Children(dev.join("Xcode/iOS DeviceSupport")),
                Src::Children(dev.join("Xcode/watchOS DeviceSupport")),
                Src::Whole(dev.join("CoreSimulator/Caches")),
                Src::Whole(h.join(".npm/_cacache")),
                Src::Whole(h.join(".gradle/caches")),
                Src::Whole(h.join(".cargo/registry/cache")),
            ],
        },
        Spec {
            name: K::CatMail,
            desc: K::CatMailDesc,
            action: Action::Purge,
            on: true,
            src: vec![Src::Children(
                l.join("Containers/com.apple.mail/Data/Library/Mail Downloads"),
            )],
        },
        Spec {
            name: K::CatInstallers,
            desc: K::CatInstallersDesc,
            action: Action::Trash,
            on: false,
            src: vec![Src::Filtered(h.join("Downloads"), is_installer)],
        },
        Spec {
            name: K::CatBackups,
            desc: K::CatBackupsDesc,
            action: Action::Trash,
            on: false,
            src: vec![Src::Children(l.join("Application Support/MobileSync/Backup"))],
        },
        Spec {
            name: K::CatTrash,
            desc: K::CatTrashDesc,
            action: Action::EmptyTrash,
            on: true,
            src: vec![Src::Children(h.join(".Trash"))],
        },
    ]
}

pub fn scan(c: &AtomicU64) -> Vec<Category> {
    specs()
        .into_par_iter()
        .filter_map(|s| {
            let mut paths: Vec<PathBuf> = Vec::new();
            for src in &s.src {
                match src {
                    Src::Children(root) => {
                        for ch in children(root) {
                            let n = file_name(&ch);
                            if NOISE.contains(&n.as_str()) || CACHE_SKIP.contains(&n.as_str()) {
                                continue;
                            }
                            paths.push(ch);
                        }
                    }
                    Src::Whole(root) => {
                        if root.exists() {
                            paths.push(root.clone());
                        }
                    }
                    Src::Filtered(root, f) => {
                        paths.extend(children(root).into_iter().filter(|p| f(p)));
                    }
                }
            }
            let mut items: Vec<Item> = paths
                .par_iter()
                .map(|p| Item { path: p.clone(), size: path_size(p, c), selected: s.on })
                .filter(|i| i.size > 0 && !(i.path.is_dir() && children(&i.path).is_empty()))
                .collect();
            items.sort_by(|a, b| b.size.cmp(&a.size));
            if items.is_empty() {
                None
            } else {
                Some(Category { name: s.name, desc: s.desc, action: s.action, items })
            }
        })
        .collect()
}

#[derive(Debug, Default)]
pub struct CleanResult {
    pub freed: u64,
    pub trashed: u64,
    pub errors: Vec<String>,
}

fn purge_item(p: &Path) -> Vec<String> {
    match std::fs::symlink_metadata(p) {
        Ok(md) if md.is_dir() => empty_dir(p),
        Ok(_) => remove_permanent(p).err().into_iter().collect(),
        Err(_) => Vec::new(),
    }
}

pub fn clean(cats: &[Category], c: &AtomicU64) -> CleanResult {
    let mut res = CleanResult::default();
    for cat in cats {
        let sel: Vec<&Item> = cat.items.iter().filter(|i| i.selected).collect();
        if sel.is_empty() {
            continue;
        }
        match cat.action {
            Action::Purge | Action::EmptyTrash => {
                let errs = Mutex::new(Vec::new());
                let freed: u64 = sel
                    .par_iter()
                    .map(|it| {
                        let e = purge_item(&it.path);
                        if !e.is_empty() {
                            errs.lock().unwrap().extend(e);
                        }
                        let left = if it.path.exists() { path_size(&it.path, c) } else { 0 };
                        it.size.saturating_sub(left)
                    })
                    .sum();
                res.freed += freed;
                let mut errs = errs.into_inner().unwrap();
                // Ο Κάδος συχνά θέλει "Πλήρη πρόσβαση δίσκου" — εναλλακτικά τον αδειάζει ο Finder.
                if cat.action == Action::EmptyTrash && !errs.is_empty() && sel.len() == cat.items.len() {
                    let ok = Command::new("osascript")
                        .args(["-e", "tell application \"Finder\" to empty trash"])
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false);
                    if ok {
                        errs.clear();
                        res.freed += cat.total().saturating_sub(freed);
                    }
                }
                res.errors.extend(errs);
            }
            Action::Trash => {
                let paths: Vec<PathBuf> = sel.iter().map(|i| i.path.clone()).collect();
                let (ok, errs) = trash_many(&paths);
                res.trashed += sel.iter().filter(|i| ok.contains(&i.path)).map(|i| i.size).sum::<u64>();
                res.errors.extend(errs);
            }
        }
    }
    res
}
