//! Δοκιμές σε ψεύτικο φάκελο χρήστη (HOME), ώστε να μην αγγίζεται τίποτα πραγματικό.

use crate::*;
use crate::i18n::K;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Τα τεστ αλλάζουν το HOME, οπότε δεν πρέπει να τρέχουν ταυτόχρονα.
static HOME_LOCK: Mutex<()> = Mutex::new(());

fn write(p: &Path, bytes: usize, fill: u8) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, vec![fill; bytes]).unwrap();
}

#[test]
fn full_flow_in_fake_home() {
    let _g = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let base = std::env::temp_dir().join(format!("katharos-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let h = base.join("home");
    std::env::set_var("HOME", &h);
    assert_eq!(home(), h);

    let l = h.join("Library");
    write(&l.join("Caches/com.google.Chrome/data_0"), 3_000_000, 1);
    write(&l.join("Caches/com.apple.bird/keep"), 1_000_000, 1); // πρέπει να αγνοηθεί
    write(&l.join("Logs/DiagnosticReports/crash.ips"), 200_000, 1);
    write(&l.join("Developer/Xcode/DerivedData/MyApp-abc/Build/x.o"), 5_000_000, 1);
    write(&h.join("Downloads/Installer.dmg"), 2_000_000, 1);
    write(&h.join("Downloads/report.pdf"), 400_000, 7);
    write(&h.join("Documents/report copy.pdf"), 400_000, 7);
    write(&h.join("Documents/different.pdf"), 400_000, 8);
    write(&h.join("Movies/big.mov"), 30_000_000, 2);
    write(&h.join(".Trash/old.zip"), 1_000_000, 1);

    let c = AtomicU64::new(0);
    let mut cats = junk::scan(&c);
    let names: Vec<K> = cats.iter().map(|c| c.name).collect();
    
    assert!(names.contains(&K::CatCache));
    assert!(names.contains(&K::CatInstallers));
    let cache = cats.iter().find(|c| c.name == K::CatCache).unwrap();
    assert!(cache.items.iter().all(|i| !i.path.ends_with("com.apple.bird")));
    // Οι εγκαταστάτες είναι αποεπιλεγμένοι από προεπιλογή
    assert_eq!(cats.iter().find(|c| c.name == K::CatInstallers).unwrap().selected_count(), 0);

    // Καθαρισμός (μόνο Purge/EmptyTrash, χωρίς τον Κάδο του συστήματος)
    for cat in &mut cats {
        if cat.action == junk::Action::Trash {
            cat.set_all(false);
        }
    }
    let r = junk::clean(&cats, &c);
    println!("freed {} errors {:?}", human(r.freed), r.errors);
    assert!(r.errors.is_empty());
    assert!(r.freed >= 9_000_000);
    assert!(l.join("Caches/com.google.Chrome").is_dir(), "ο φάκελος cache μένει, αδειάζει μόνο");
    assert!(!l.join("Caches/com.google.Chrome/data_0").exists());
    assert!(l.join("Caches/com.apple.bird/keep").exists());
    assert!(!h.join(".Trash/old.zip").exists());
    assert!(h.join("Downloads/Installer.dmg").exists());
    assert!(h.join("Documents/different.pdf").exists());

    // Δίχτυ ασφαλείας
    assert!(!is_safe_target(&h));
    assert!(!is_safe_target(&h.join("Documents")));
    assert!(!is_safe_target(&l.join("Caches")));
    assert!(!is_safe_target(Path::new("/etc/hosts")));
    assert!(!is_safe_target(Path::new("/Applications")));
    assert!(is_safe_target(Path::new("/Applications/Foo.app")));
    assert!(is_safe_target(&h.join("Documents/a.pdf")));
    assert!(remove_permanent(&h.join("Documents")).is_err());
    assert!(h.join("Documents").exists());

    // Μεγάλα αρχεία
    let large = files::scan_large(10_000_000, &c);
    assert_eq!(large.len(), 1);
    assert!(large[0].path.ends_with("Movies/big.mov"));

    // Διπλότυπα: ίδιο περιεχόμενο → μία ομάδα, όχι το "different.pdf"
    let d = files::scan_dupes(100_000, &c);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].files.len(), 2);

    // Κατάλοιπα εφαρμογών
    assert!(apps::matches("com.spotify.client", "com.spotify.client", "Spotify", false));
    assert!(apps::matches("com.spotify.client.plist", "com.spotify.client", "Spotify", false));
    assert!(apps::matches("ABCDE.com.spotify.client", "com.spotify.client", "Spotify", false));
    assert!(apps::matches("Spotify", "com.spotify.client", "Spotify", true));
    assert!(!apps::matches("Spotify", "com.spotify.client", "Spotify", false));
    assert!(!apps::matches("com.spotify.clientx", "com.spotify.client", "Spotify", false));
    assert!(!apps::matches("Zoom", "us.zoom.xos", "Zoo", true));

    // Χάρτης χώρου
    let e = lens::list(&h, &c);
    assert_eq!(e[0].name, "Movies");

    fs::remove_dir_all(&base).unwrap();
}

// ───────────────────────── Προστασία ─────────────────────────

#[test]
fn protect_parsers() {
    use crate::protect::*;
    let (p, th) = parse_clam_line("/Users/a/Downloads/x.dmg: Osx.Trojan.Agent-1 FOUND\n").unwrap();
    assert_eq!(p, PathBuf::from("/Users/a/Downloads/x.dmg"));
    assert_eq!(th, "Osx.Trojan.Agent-1");
    assert!(parse_clam_line("/a/b: OK").is_none());
    let (p, _) = parse_clam_line("/a/we: ird: name.txt: X FOUND").unwrap();
    assert_eq!(p, PathBuf::from("/a/we: ird: name.txt"));

    assert_eq!(parse_codesign("/x: code object is not signed at all"), Sig::Unsigned);
    assert_eq!(parse_codesign("Executable=/x\nSignature=adhoc\nTeamIdentifier=not set"), Sig::Adhoc);
    assert_eq!(
        parse_codesign("Authority=Developer ID Application: Google LLC (EQHXZ8M8AV)\nAuthority=Developer ID Certification Authority"),
        Sig::DevId("Google LLC (EQHXZ8M8AV)".into())
    );
    assert_eq!(parse_codesign("Authority=Apple Mac OS Application Signing\n"), Sig::AppStore);
    assert_eq!(parse_codesign("Authority=Software Signing\nAuthority=Apple Code Signing Certification Authority"), Sig::Apple);
    assert_eq!(parse_codesign(""), Sig::Unknown);

    assert_eq!(parse_spctl("/Applications/A.app: accepted\nsource=Notarized Developer ID\n"), Gk::Notarized);
    assert_eq!(parse_spctl("/Applications/A.app: accepted\nsource=Mac App Store\n"), Gk::AppStore);
    assert_eq!(parse_spctl("/Applications/A.app: rejected\nsource=no usable signature\n"), Gk::Rejected);
    assert_eq!(parse_spctl("/Applications/A.app: accepted\nsource=Developer ID\n"), Gk::Accepted);

    let pk = "+    com.adblock.Safari.Extension(2.1)\n\t            Path = /Applications/AdBlock.app/Contents/PlugIns/Ext.appex\n\t    Display Name = AdBlock Ext\n-    com.other.ext(1.0)\n\t            Path = /Applications/O.app/Contents/PlugIns/O.appex\n";
    let v = parse_pluginkit(pk);
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].0, "com.adblock.Safari.Extension");
    assert_eq!(v[0].1, "AdBlock Ext");
    assert!(v[0].3);
    assert_eq!(v[1].2, PathBuf::from("/Applications/O.app/Contents/PlugIns/O.appex"));
    assert!(!v[1].3);
}

fn hex(s: &str) -> String {
    s.bytes().map(|b| format!("{:02x}", b)).collect()
}

#[test]
fn protect_flow_in_fake_home() {
    use crate::protect::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicU64, Ordering};
    let _g = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let base = std::env::temp_dir().join(format!("katharos-protect-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let h = base.join("home");
    std::env::set_var("HOME", &h);

    // ── Καραντίνα: μετακίνηση, επαναφορά με τα ίδια δικαιώματα, οριστική διαγραφή
    let bad = h.join("Downloads/bad.bin");
    write(&bad, 1000, 9);
    fs::set_permissions(&bad, fs::Permissions::from_mode(0o755)).unwrap();
    quarantine_one(&bad, "Test.Threat").unwrap();
    assert!(!bad.exists());
    let q = list_quarantine();
    assert_eq!(q.len(), 1);
    assert_eq!(q[0].original, bad);
    assert_eq!(q[0].threat, "Test.Threat");
    assert_eq!(fs::metadata(&q[0].file).unwrap().permissions().mode() & 0o777, 0o400);
    restore(&q[0]).unwrap();
    assert!(bad.exists());
    assert_eq!(fs::metadata(&bad).unwrap().permissions().mode() & 0o777, 0o755);
    assert!(list_quarantine().is_empty());
    quarantine_one(&bad, "Test.Threat").unwrap();
    delete_quarantined(&list_quarantine()[0]).unwrap();
    assert!(list_quarantine().is_empty() && !bad.exists());
    assert!(quarantine_one(&h.join("Downloads"), "x").is_err(), "δεν μπαίνουν φάκελοι σε καραντίνα");
    assert!(quarantine_one(Path::new("/usr/bin/env"), "x").is_err(), "ούτε αρχεία συστήματος");

    // ── Αυτόματη εκκίνηση
    let la = h.join("Library/LaunchAgents");
    fs::create_dir_all(&la).unwrap();
    let mut d = plist::Dictionary::new();
    d.insert("Label".into(), "com.evil.updater".into());
    d.insert(
        "ProgramArguments".into(),
        plist::Value::Array(vec!["/tmp/.hidden/updater".into(), "--silent".into()]),
    );
    plist::Value::Dictionary(d).to_file_xml(la.join("com.evil.updater.plist")).unwrap();
    let mut d = plist::Dictionary::new();
    d.insert("Label".into(), "com.script.thing".into());
    d.insert("ProgramArguments".into(), plist::Value::Array(vec!["/bin/sh".into(), "-c".into(), "echo hi".into()]));
    plist::Value::Dictionary(d).to_file_xml(la.join("com.script.thing.plist")).unwrap();
    let items = startup_items();
    let evil = items.iter().find(|i| i.label == "com.evil.updater").unwrap();
    assert!(evil.flags.contains(&K::FlagMissing));
    assert!(evil.flags.contains(&K::FlagHidden));
    assert!(evil.flags.contains(&K::FlagTemp));
    assert!(!evil.system);
    assert_eq!(items[0].label, "com.evil.updater", "τα πιο ύποπτα πρώτα");
    let sh = items.iter().find(|i| i.label == "com.script.thing").unwrap();
    assert!(sh.flags.contains(&K::FlagScript));

    // ── Επέκταση Chrome με τοπικοποιημένο όνομα και επικίνδυνα δικαιώματα
    let ext = h.join("Library/Application Support/Google/Chrome/Default/Extensions/abcdef/1.2_0");
    fs::create_dir_all(ext.join("_locales/en")).unwrap();
    fs::write(
        ext.join("manifest.json"),
        r#"{"name":"__MSG_appName__","version":"1.2","default_locale":"en","permissions":["tabs","cookies","storage"],"host_permissions":["<all_urls>"]}"#,
    )
    .unwrap();
    fs::write(ext.join("_locales/en/messages.json"), r#"{"appName":{"message":"Coupon Helper"}}"#).unwrap();
    let exts = chromium_exts(&h.join("Library/Application Support/Google/Chrome"), "Chrome");
    assert_eq!(exts.len(), 1);
    assert_eq!(exts[0].name, "Coupon Helper");
    assert_eq!(exts[0].browser, "Chrome");
    assert!(exts[0].all_sites);
    assert_eq!(exts[0].sensitive, vec!["cookies".to_string(), "tabs".to_string()]);

    // ── Πραγματική σάρωση με ClamAV (αν υπάρχει) και δική μας δοκιμαστική υπογραφή
    if find_bin("clamscan").is_some() {
        let marker = "KATHAROS-HARMLESS-TEST-MARKER-7f3e";
        fs::create_dir_all(db_dir()).unwrap();
        fs::write(db_dir().join("katharos-test.ndb"), format!("Katharos.Test.Marker:0:*:{}\n", hex(marker))).unwrap();
        fs::write(h.join("Downloads/clean.txt"), "hello").unwrap();
        fs::write(h.join("Downloads/infected.txt"), format!("abc {} xyz", marker)).unwrap();
        fs::create_dir_all(h.join("Desktop/deep/er")).unwrap();
        fs::write(h.join("Desktop/deep/er/also infected.txt"), marker).unwrap();
        let done = AtomicU64::new(0);
        let total = AtomicU64::new(0);
        let ctl = ScanCtl::default();
        let r = scan(&scan_targets(ScanKind::Quick), &done, &total, &ctl);
        println!("{:?}", r);
        assert!(r.error.is_none());
        assert!(!r.cancelled);
        assert_eq!(r.total, 3 + 2, "3 αρχεία + τα 2 plist αυτόματης εκκίνησης");
        assert_eq!(r.scanned, r.total);
        let mut found: Vec<String> = r.infected.iter().map(|i| file_name(&i.path)).collect();
        found.sort();
        assert_eq!(found, vec!["also infected.txt".to_string(), "infected.txt".to_string()]);
        assert!(r.infected.iter().all(|i| i.threat.starts_with("Katharos.Test.Marker")));
        assert_eq!(total.load(Ordering::Relaxed), 5);
        // καραντίνα του ευρήματος
        for i in &r.infected {
            quarantine_one(&i.path, &i.threat).unwrap();
        }
        assert_eq!(list_quarantine().len(), 2);
        // Η επόμενη σάρωση δεν ξανασαρώνει την καραντίνα (βρίσκεται μέσα στον HOME)
        let r2 = scan(&[h.clone()], &AtomicU64::new(0), &AtomicU64::new(0), &ScanCtl::default());
        assert!(r2.infected.is_empty(), "{:?}", r2.infected);
    } else {
        println!("clamscan not installed — real scan skipped");
    }
    fs::remove_dir_all(&base).unwrap();
}
