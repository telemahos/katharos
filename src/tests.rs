//! Δοκιμές σε ψεύτικο φάκελο χρήστη (HOME), ώστε να μην αγγίζεται τίποτα πραγματικό.

use crate::*;
use crate::i18n::K;
use std::fs;
use std::path::Path;

fn write(p: &Path, bytes: usize, fill: u8) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, vec![fill; bytes]).unwrap();
}

#[test]
fn full_flow_in_fake_home() {
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
