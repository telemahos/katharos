//! Μεταφράσεις. Κάθε κείμενο της εφαρμογής είναι ένα `K` με 7 γλώσσες.
//! Η σειρά στις γλώσσες είναι πάντα: el, en, de, es, it, fr, zh.
//! Η επιλεγμένη γλώσσα είναι καθολική, ώστε κάθε μέρος του κώδικα να την διαβάζει
//! με `t(K::..)` χωρίς να τη μεταφέρει παντού.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    El = 0,
    En = 1,
    De = 2,
    Es = 3,
    It = 4,
    Fr = 5,
    Zh = 6,
}

pub const ALL: [Lang; 7] = [Lang::El, Lang::En, Lang::De, Lang::Es, Lang::It, Lang::Fr, Lang::Zh];

impl Lang {
    /// Όνομα της γλώσσας, γραμμένο στην ίδια τη γλώσσα.
    pub fn name(self) -> &'static str {
        match self {
            Lang::El => "Ελληνικά",
            Lang::En => "English",
            Lang::De => "Deutsch",
            Lang::Es => "Español",
            Lang::It => "Italiano",
            Lang::Fr => "Français",
            Lang::Zh => "中文",
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(Lang::En as u8);

pub fn set_lang(l: Lang) {
    CURRENT.store(l as u8, Ordering::Relaxed);
}

pub fn lang() -> Lang {
    ALL[(CURRENT.load(Ordering::Relaxed) as usize) % ALL.len()]
}

pub fn from_code(code: &str) -> Option<Lang> {
    let c = code.split(['_', '-', '.', '@']).next()?.to_lowercase();
    match c.as_str() {
        "el" => Some(Lang::El),
        "en" => Some(Lang::En),
        "de" => Some(Lang::De),
        "es" => Some(Lang::Es),
        "it" => Some(Lang::It),
        "fr" => Some(Lang::Fr),
        "zh" => Some(Lang::Zh),
        _ => None,
    }
}

/// Η γλώσσα του Mac (AppleLocale), με εναλλακτική τη μεταβλητή LANG.
pub fn detect() -> Lang {
    let from_defaults = std::process::Command::new("defaults")
        .args(["read", "-g", "AppleLocale"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let raw = if from_defaults.is_empty() {
        std::env::var("LANG").unwrap_or_default()
    } else {
        from_defaults
    };
    from_code(&raw).unwrap_or(Lang::En)
}

/// Κείμενο στην τρέχουσα γλώσσα.
pub fn t(k: K) -> &'static str {
    row(k)[lang() as usize]
}

/// Κείμενο με προτύπου `{}`, που αντικαθίστανται με τα `args` με τη σειρά.
pub fn tf(k: K, args: &[&str]) -> String {
    let mut out = String::new();
    let mut parts = args.iter();
    let mut rest = t(k);
    while let Some(pos) = rest.find("{}") {
        out.push_str(&rest[..pos]);
        out.push_str(parts.next().copied().unwrap_or(""));
        rest = &rest[pos + 2..];
    }
    out.push_str(rest);
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum K {
    NavSmart,
    NavLarge,
    NavDupes,
    NavUninstall,
    NavLens,
    NavMaint,
    Tagline,
    Language,
    Disk,
    FreedToday,
    FreeOf,
    FoundSummary,
    Freed,
    TrashedSuffix,
    Rechecking,
    DupeSummary,
    AppsCount,
    LargeFound,
    MovedToTrash,
    ErrorsCount,
    ErrorsHelp,
    Busy,
    ItemsChecked,
    ItemsCount,
    CliSummary,
    Scan,
    Rescan,
    Clean,
    SmartIntro,
    SmartSub,
    SelectedForCleaning,
    OfFound,
    MacClean,
    ItemsSelectedOf,
    AndMore,
    ClickReveal,
    CleanBody,
    TrashEmptyNote,
    Cancel,
    MoveToTrash,
    TrashLargeBody,
    TrashDupesBody,
    TrashOneBody,
    UninstallTitle,
    UninstallBody,
    Uninstall,
    UninstallBtn,
    ProtectedSkip,
    AgeToday,
    AgeYesterday,
    AgeDays,
    AgeMonths,
    AgeYears,
    LargeTitle,
    LargeSub,
    LargeIntro,
    LargeSearching,
    MinSize,
    Unused,
    AgeAll,
    Age3m,
    Age6m,
    Age1y,
    LargeSel,
    SelectAll,
    SelectNone,
    ShowInFinder,
    DupesSub,
    DupesIntro,
    FindDupes,
    Comparing,
    DupesSel,
    DupGroupLine,
    Original,
    NoDupes,
    UninstallSub,
    ReadingApps,
    SearchHint,
    PickApp,
    Version,
    AppLabel,
    LeftoversLabel,
    RelatedFiles,
    NotFoundSearching,
    LensTitle,
    LensSub,
    Up,
    Home,
    Applications,
    Library,
    Refresh,
    Measuring,
    Total,
    MaintSub,
    Run,
    MRamName,
    MRamDesc,
    MDnsName,
    MDnsDesc,
    MSpotName,
    MSpotDesc,
    MQLName,
    MQLDesc,
    MLsName,
    MLsDesc,
    MDone,
    MCancel,
    CatCache,
    CatCacheDesc,
    CatLogs,
    CatLogsDesc,
    CatState,
    CatStateDesc,
    CatXcode,
    CatXcodeDesc,
    CatMail,
    CatMailDesc,
    CatInstallers,
    CatInstallersDesc,
    CatBackups,
    CatBackupsDesc,
    CatTrash,
    CatTrashDesc,
}

/// Σειρά: el, en, de, es, it, fr, zh
fn row(k: K) -> [&'static str; 7] {
    use K::*;
    match k {
        NavSmart => ["Έξυπνη Σάρωση", "Smart Scan", "Smart-Scan", "Análisis inteligente", "Scansione intelligente", "Analyse intelligente", "智能扫描"],
        NavLarge => ["Μεγάλα & Παλιά", "Large & Old", "Große & alte", "Grandes y antiguos", "Grandi e vecchi", "Gros & anciens", "大文件与旧文件"],
        NavDupes => ["Διπλότυπα", "Duplicates", "Duplikate", "Duplicados", "Duplicati", "Doublons", "重复文件"],
        NavUninstall => ["Απεγκατάσταση", "Uninstaller", "Deinstallation", "Desinstalador", "Disinstallazione", "Désinstallation", "卸载应用"],
        NavLens => ["Χάρτης Χώρου", "Space Lens", "Speicher-Lupe", "Lupa de espacio", "Lente dello spazio", "Loupe d'espace", "空间透镜"],
        NavMaint => ["Συντήρηση", "Maintenance", "Wartung", "Mantenimiento", "Manutenzione", "Maintenance", "系统维护"],
        Tagline => ["Καθαριστής για Mac", "Mac cleaner", "Mac-Reiniger", "Limpiador para Mac", "Pulizia per Mac", "Nettoyeur pour Mac", "Mac 清理工具"],
        Language => ["Γλώσσα", "Language", "Sprache", "Idioma", "Lingua", "Langue", "语言"],
        Disk => ["Δίσκος", "Disk", "Speicher", "Disco", "Disco", "Disque", "磁盘"],
        FreedToday => ["Ελευθέρωσες {} σήμερα", "Freed {} today", "{} heute freigegeben", "Liberaste {} hoy", "Liberati {} oggi", "{} libérés aujourd'hui", "今天已释放 {}"],
        FreeOf => ["{} ελεύθερα από {}", "{} free of {}", "{} frei von {}", "{} libres de {}", "{} liberi su {}", "{} libres sur {}", "剩余 {}，共 {}"],
        FoundSummary => ["Βρέθηκαν {} σε {} κατηγορίες", "Found {} in {} categories", "{} in {} Kategorien gefunden", "Encontrados {} en {} categorías", "Trovati {} in {} categorie", "{} trouvés dans {} catégories", "找到 {}，分为 {} 类"],
        Freed => ["Ελευθερώθηκαν {}", "Freed {}", "{} freigegeben", "Liberados {}", "Liberati {}", "{} libérés", "已释放 {}"],
        TrashedSuffix => [" · {} μετακινήθηκαν στον Κάδο", " · {} moved to Trash", " · {} in den Papierkorb verschoben", " · {} movidos a la papelera", " · {} spostati nel Cestino", " · {} déplacés dans la Corbeille", " · {} 已移至废纸篓"],
        Rechecking => ["Επανέλεγχος", "Rechecking", "Erneut prüfen", "Revisando", "Ricontrollo", "Nouvelle vérification", "重新检查"],
        DupeSummary => ["{} ομάδες διπλοτύπων · {} περιττός χώρος", "{} duplicate groups · {} wasted space", "{} Dubletten-Gruppen · {} verschwendeter Speicher", "{} grupos de duplicados · {} espacio desperdiciado", "{} gruppi di duplicati · {} spazio sprecato", "{} groupes de doublons · {} espace gaspillé", "{} 组重复文件 · 浪费 {}"],
        AppsCount => ["{} εφαρμογές", "{} apps", "{} Apps", "{} aplicaciones", "{} app", "{} applications", "{} 个应用"],
        LargeFound => ["Βρέθηκαν {} μεγάλα αρχεία", "Found {} large files", "{} große Dateien gefunden", "Encontrados {} archivos grandes", "Trovati {} file grandi", "{} gros fichiers trouvés", "找到 {} 个大文件"],
        MovedToTrash => ["{} αντικείμενα ({}) μετακινήθηκαν στον Κάδο", "{} items ({}) moved to Trash", "{} Objekte ({}) in den Papierkorb verschoben", "{} elementos ({}) movidos a la papelera", "{} elementi ({}) spostati nel Cestino", "{} éléments ({}) déplacés dans la Corbeille", "{} 项（{}）已移至废纸篓"],
        ErrorsCount => ["⚠ {} δεν διαγράφηκαν", "⚠ {} not deleted", "⚠ {} nicht gelöscht", "⚠ {} no eliminados", "⚠ {} non eliminati", "⚠ {} non supprimés", "⚠ {} 项未删除"],
        ErrorsHelp => [
            "Συνήθως αρχεία σε χρήση ή προστατευμένα. Για περισσότερα, δώσε «Πλήρη πρόσβαση δίσκου» στο Katharos (Ρυθμίσεις συστήματος → Απόρρητο & ασφάλεια).",
            "Usually files in use or protected. For more, grant Katharos «Full Disk Access» (System Settings → Privacy & Security).",
            "Meist Dateien in Benutzung oder geschützt. Für mehr gib Katharos den „Vollen Festplattenzugriff“ (Systemeinstellungen → Datenschutz & Sicherheit).",
            "Normalmente archivos en uso o protegidos. Para más, concede a Katharos «Acceso completo al disco» (Ajustes del Sistema → Privacidad y seguridad).",
            "Di solito file in uso o protetti. Per averne di più, concedi a Katharos «Accesso completo al disco» (Impostazioni di Sistema → Privacy e sicurezza).",
            "Généralement des fichiers utilisés ou protégés. Pour en supprimer davantage, accorde à Katharos « Accès complet au disque » (Réglages Système → Confidentialité et sécurité).",
            "通常是正在使用或受保护的文件。如需更多权限，请在“系统设置 → 隐私与安全性”中为 Katharos 授予“完全磁盘访问权限”。",
        ],
        Busy => ["{}… {} αντικείμενα", "{}… {} items", "{}… {} Objekte", "{}… {} elementos", "{}… {} elementi", "{}… {} éléments", "{}… 已检查 {} 项"],
        ItemsChecked => ["{} αντικείμενα ελέγχθηκαν", "{} items checked", "{} Objekte geprüft", "{} elementos revisados", "{} elementi controllati", "{} éléments vérifiés", "已检查 {} 项"],
        ItemsCount => ["{} στοιχεία", "{} items", "{} Objekte", "{} elementos", "{} elementi", "{} éléments", "{} 项"],
        CliSummary => ["Προτείνεται καθαρισμός: {}  —  {} αντικείμενα σε {}s", "Suggested cleanup: {}  —  {} items in {}s", "Empfohlene Bereinigung: {}  —  {} Objekte in {}s", "Limpieza sugerida: {}  —  {} elementos en {}s", "Pulizia consigliata: {}  —  {} elementi in {}s", "Nettoyage suggéré : {}  —  {} éléments en {}s", "建议清理：{}  —  {} 项，用时 {}s"],
        Scan => ["Σάρωση", "Scan", "Scannen", "Escanear", "Scansiona", "Analyser", "扫描"],
        Rescan => ["↻ Νέα σάρωση", "↻ Scan again", "↻ Neu scannen", "↻ Escanear de nuevo", "↻ Scansiona di nuovo", "↻ Relancer l'analyse", "↻ 重新扫描"],
        Clean => ["Καθαρισμός", "Clean", "Bereinigen", "Limpiar", "Pulisci", "Nettoyer", "清理"],
        SmartIntro => ["Έλεγξε το Mac σου για περιττά αρχεία.", "Check your Mac for unnecessary files.", "Prüfe deinen Mac auf unnötige Dateien.", "Busca archivos innecesarios en tu Mac.", "Cerca file inutili sul tuo Mac.", "Recherche les fichiers inutiles sur ton Mac.", "检查 Mac 上的无用文件。"],
        SmartSub => ["Βρίσκει cache, logs, κατάλοιπα και σκουπίδια που διαγράφονται με ασφάλεια.", "Finds caches, logs, leftovers and junk that can be safely deleted.", "Findet Caches, Protokolle, Reste und Datenmüll, die sicher gelöscht werden können.", "Encuentra cachés, registros, restos y basura que se pueden borrar con seguridad.", "Trova cache, log, residui e file spazzatura eliminabili in sicurezza.", "Trouve les caches, journaux, restes et déchets supprimables en toute sécurité.", "查找可以安全删除的缓存、日志、残留文件和垃圾文件。"],
        SelectedForCleaning => ["επιλεγμένα για καθαρισμό", "selected for cleaning", "zum Bereinigen ausgewählt", "seleccionados para limpiar", "selezionati per la pulizia", "sélectionnés pour le nettoyage", "已选中待清理"],
        OfFound => ["από {} που βρέθηκαν", "out of {} found", "von {} gefunden", "de {} encontrados", "su {} trovati", "sur {} trouvés", "共找到 {}"],
        MacClean => ["Το Mac σου είναι καθαρό! 🎉", "Your Mac is clean! 🎉", "Dein Mac ist sauber! 🎉", "¡Tu Mac está limpio! 🎉", "Il tuo Mac è pulito! 🎉", "Ton Mac est propre ! 🎉", "你的 Mac 很干净！🎉"],
        ItemsSelectedOf => ["{} από {} στοιχεία", "{} of {} items", "{} von {} Objekten", "{} de {} elementos", "{} di {} elementi", "{} sur {} éléments", "已选 {} / 共 {} 项"],
        AndMore => ["…και {} ακόμη", "…and {} more", "…und {} weitere", "…y {} más", "…e altri {}", "…et {} de plus", "…另外 {} 项"],
        ClickReveal => ["Κλικ: εμφάνιση στο Finder", "Click: show in Finder", "Klicken: im Finder zeigen", "Clic: mostrar en el Finder", "Clic: mostra nel Finder", "Clic : afficher dans le Finder", "点击：在访达中显示"],
        CleanBody => [
            "Θα καθαριστούν {}. Τα cache και logs διαγράφονται οριστικά (οι εφαρμογές τα ξαναφτιάχνουν).{}\n\nΚλείσε πρώτα τα ανοιχτά προγράμματα περιήγησης για καλύτερο αποτέλεσμα.",
            "{} will be cleaned. Caches and logs are deleted permanently (apps recreate them).{}\n\nClose open browsers first for best results.",
            "{} werden bereinigt. Caches und Protokolle werden endgültig gelöscht (Apps erstellen sie neu).{}\n\nSchließe vorher geöffnete Browser für das beste Ergebnis.",
            "Se limpiarán {}. Los cachés y registros se borran de forma permanente (las apps los vuelven a crear).{}\n\nCierra antes los navegadores abiertos para mejores resultados.",
            "Verranno puliti {}. Cache e log vengono eliminati definitivamente (le app li ricreano).{}\n\nChiudi prima i browser aperti per risultati migliori.",
            "{} seront nettoyés. Les caches et journaux sont supprimés définitivement (les applications les recréent).{}\n\nFerme d'abord les navigateurs ouverts pour de meilleurs résultats.",
            "将清理 {}。缓存和日志会被永久删除（应用会自动重新生成）。{}\n\n建议先关闭打开的浏览器以获得更好的效果。",
        ],
        TrashEmptyNote => ["\nΟ Κάδος θα αδειάσει οριστικά.", "\nThe Trash will be emptied permanently.", "\nDer Papierkorb wird endgültig geleert.", "\nLa papelera se vaciará de forma permanente.", "\nIl Cestino verrà svuotato definitivamente.", "\nLa Corbeille sera vidée définitivement.", "\n废纸篓将被永久清空。"],
        MoveToTrash => ["Μετακίνηση στον Κάδο", "Move to Trash", "In den Papierkorb verschieben", "Mover a la papelera", "Sposta nel Cestino", "Déplacer dans la Corbeille", "移至废纸篓"],
        Cancel => ["Ακύρωση", "Cancel", "Abbrechen", "Cancelar", "Annulla", "Annuler", "取消"],
        TrashLargeBody => ["{} αρχεία θα μετακινηθούν στον Κάδο. Μπορείς να τα επαναφέρεις από εκεί.", "{} files will be moved to the Trash. You can restore them from there.", "{} Dateien werden in den Papierkorb verschoben. Du kannst sie von dort wiederherstellen.", "{} archivos se moverán a la papelera. Puedes restaurarlos desde allí.", "{} file verranno spostati nel Cestino. Puoi ripristinarli da lì.", "{} fichiers seront déplacés dans la Corbeille. Tu pourras les restaurer depuis là.", "{} 个文件将移至废纸篓，可以从那里恢复。"],
        TrashDupesBody => ["{} διπλότυπα θα μετακινηθούν στον Κάδο. Σε κάθε ομάδα κρατιέται τουλάχιστον ένα αντίγραφο.", "{} duplicates will be moved to the Trash. At least one copy is kept in each group.", "{} Duplikate werden in den Papierkorb verschoben. In jeder Gruppe bleibt mindestens eine Kopie erhalten.", "{} duplicados se moverán a la papelera. En cada grupo se conserva al menos una copia.", "{} duplicati verranno spostati nel Cestino. In ogni gruppo viene conservata almeno una copia.", "{} doublons seront déplacés dans la Corbeille. Au moins une copie est conservée dans chaque groupe.", "{} 个重复文件将移至废纸篓。每组至少保留一个副本。"],
        TrashOneBody => ["{} ({}) θα μετακινηθεί στον Κάδο.", "{} ({}) will be moved to the Trash.", "{} ({}) wird in den Papierkorb verschoben.", "{} ({}) se moverá a la papelera.", "{} ({}) verrà spostato nel Cestino.", "{} ({}) sera déplacé dans la Corbeille.", "{}（{}）将移至废纸篓。"],
        UninstallTitle => ["Απεγκατάσταση {}", "Uninstall {}", "{} deinstallieren", "Desinstalar {}", "Disinstalla {}", "Désinstaller {}", "卸载 {}"],
        UninstallBody => ["Το {} θα κλείσει και, μαζί με τα επιλεγμένα κατάλοιπα, θα μετακινηθεί στον Κάδο.", "{} will quit and, together with the selected leftovers, be moved to the Trash.", "{} wird beendet und zusammen mit den ausgewählten Resten in den Papierkorb verschoben.", "{} se cerrará y, junto con los restos seleccionados, se moverá a la papelera.", "{} verrà chiuso e, insieme ai residui selezionati, spostato nel Cestino.", "{} sera fermé et, avec les restes sélectionnés, déplacé dans la Corbeille.", "{} 将退出，并与所选残留文件一起移至废纸篓。"],
        Uninstall => ["Απεγκατάσταση", "Uninstall", "Deinstallieren", "Desinstalar", "Disinstalla", "Désinstaller", "卸载"],
        UninstallBtn => ["Απεγκατάσταση ({})", "Uninstall ({})", "Deinstallieren ({})", "Desinstalar ({})", "Disinstalla ({})", "Désinstaller ({})", "卸载（{}）"],
        ProtectedSkip => ["Προστατευμένη διαδρομή, παραλείφθηκε: {}", "Protected path, skipped: {}", "Geschützter Pfad, übersprungen: {}", "Ruta protegida, omitida: {}", "Percorso protetto, saltato: {}", "Chemin protégé, ignoré : {}", "受保护的路径，已跳过：{}"],
        AgeToday => ["σήμερα", "today", "heute", "hoy", "oggi", "aujourd'hui", "今天"],
        AgeYesterday => ["χθες", "yesterday", "gestern", "ayer", "ieri", "hier", "昨天"],
        AgeDays => ["πριν {} ημέρες", "{} days ago", "vor {} Tagen", "hace {} días", "{} giorni fa", "il y a {} jours", "{} 天前"],
        AgeMonths => ["πριν {} μήνες", "{} months ago", "vor {} Monaten", "hace {} meses", "{} mesi fa", "il y a {} mois", "{} 个月前"],
        AgeYears => ["πριν {} χρόνια", "{} years ago", "vor {} Jahren", "hace {} años", "{} anni fa", "il y a {} ans", "{} 年前"],
        LargeTitle => ["Μεγάλα & Παλιά αρχεία", "Large & Old Files", "Große & alte Dateien", "Archivos grandes y antiguos", "File grandi e vecchi", "Gros et anciens fichiers", "大文件与旧文件"],
        LargeSub => ["Αρχεία στον φάκελό σου που πιάνουν πολύ χώρο ή έχεις καιρό να ανοίξεις.", "Files that take up a lot of space or that you haven't opened in a long time.", "Dateien, die viel Speicher belegen oder die du lange nicht geöffnet hast.", "Archivos que ocupan mucho espacio o que hace tiempo no abres.", "File che occupano molto spazio o che non apri da tempo.", "Fichiers qui prennent beaucoup de place ou que tu n'as pas ouverts depuis longtemps.", "占用大量空间或很久未打开的文件。"],
        LargeIntro => ["Βρες τα αρχεία που τρώνε τον χώρο σου.", "Find the files that are using up your space.", "Finde die Dateien, die deinen Speicher belegen.", "Encuentra los archivos que ocupan tu espacio.", "Trova i file che occupano il tuo spazio.", "Trouve les fichiers qui prennent ta place.", "找出占用你空间的文件。"],
        LargeSearching => ["Αναζήτηση μεγάλων αρχείων", "Searching for large files", "Große Dateien werden gesucht", "Buscando archivos grandes", "Ricerca di file grandi", "Recherche de gros fichiers", "正在搜索大文件"],
        MinSize => ["Ελάχιστο μέγεθος:", "Minimum size:", "Mindestgröße:", "Tamaño mínimo:", "Dimensione minima:", "Taille minimale :", "最小大小："],
        Unused => ["Αχρησιμοποίητα:", "Unused:", "Ungenutzt:", "Sin uso:", "Non usati:", "Inutilisés :", "未使用："],
        AgeAll => ["όλα", "all", "alle", "todos", "tutti", "tous", "全部"],
        Age3m => ["> 3 μήνες", "> 3 months", "> 3 Monate", "> 3 meses", "> 3 mesi", "> 3 mois", "超过 3 个月"],
        Age6m => ["> 6 μήνες", "> 6 months", "> 6 Monate", "> 6 meses", "> 6 mesi", "> 6 mois", "超过 6 个月"],
        Age1y => ["> 1 χρόνο", "> 1 year", "> 1 Jahr", "> 1 año", "> 1 anno", "> 1 an", "超过 1 年"],
        LargeSel => ["{} αρχεία · επιλεγμένα {} ({})", "{} files · selected {} ({})", "{} Dateien · ausgewählt {} ({})", "{} archivos · seleccionados {} ({})", "{} file · selezionati {} ({})", "{} fichiers · sélectionnés {} ({})", "{} 个文件 · 已选 {}（{}）"],
        SelectAll => ["Επιλογή όλων", "Select all", "Alle auswählen", "Seleccionar todo", "Seleziona tutto", "Tout sélectionner", "全选"],
        SelectNone => ["Καμία", "None", "Keine", "Ninguno", "Nessuno", "Aucun", "全不选"],
        ShowInFinder => ["Εμφάνιση στο Finder", "Show in Finder", "Im Finder zeigen", "Mostrar en el Finder", "Mostra nel Finder", "Afficher dans le Finder", "在访达中显示"],
        DupesSub => ["Ίδια αρχεία (σύγκριση περιεχομένου BLAKE3) σε Επιφάνεια εργασίας, Έγγραφα, Λήψεις, Εικόνες, Ταινίες, Μουσική.", "Identical files (content comparison with BLAKE3) in Desktop, Documents, Downloads, Pictures, Movies and Music.", "Identische Dateien (Inhaltsvergleich mit BLAKE3) in Schreibtisch, Dokumente, Downloads, Bilder, Filme und Musik.", "Archivos idénticos (comparación de contenido con BLAKE3) en Escritorio, Documentos, Descargas, Imágenes, Películas y Música.", "File identici (confronto del contenuto con BLAKE3) in Scrivania, Documenti, Download, Immagini, Film e Musica.", "Fichiers identiques (comparaison du contenu avec BLAKE3) dans Bureau, Documents, Téléchargements, Images, Films et Musique.", "通过 BLAKE3 比较内容，查找“桌面”“文稿”“下载”“图片”“影片”“音乐”中的相同文件。"],
        DupesIntro => ["Το παλαιότερο αντίγραφο κρατιέται πάντα.", "The oldest copy is always kept.", "Die älteste Kopie bleibt immer erhalten.", "La copia más antigua siempre se conserva.", "La copia più vecchia viene sempre conservata.", "La copie la plus ancienne est toujours conservée.", "始终保留最早的副本。"],
        FindDupes => ["Αναζήτηση διπλοτύπων", "Find duplicates", "Duplikate suchen", "Buscar duplicados", "Cerca duplicati", "Rechercher les doublons", "查找重复文件"],
        Comparing => ["Σύγκριση αρχείων", "Comparing files", "Dateien werden verglichen", "Comparando archivos", "Confronto dei file", "Comparaison des fichiers", "正在比较文件"],
        DupesSel => ["{} ομάδες · επιλεγμένα {} ({})", "{} groups · selected {} ({})", "{} Gruppen · ausgewählt {} ({})", "{} grupos · seleccionados {} ({})", "{} gruppi · selezionati {} ({})", "{} groupes · sélectionnés {} ({})", "{} 组 · 已选 {}（{}）"],
        DupGroupLine => ["{} — {} αντίγραφα × {} · περιττά {}", "{} — {} copies × {} · redundant {}", "{} — {} Kopien × {} · überflüssig {}", "{} — {} copias × {} · sobrantes {}", "{} — {} copie × {} · in eccesso {}", "{} — {} copies × {} · en trop {}", "{} — {} 份副本 × {} · 多余 {}"],
        Original => ["πρωτότυπο", "original", "Original", "original", "originale", "original", "原件"],
        NoDupes => ["Δεν βρέθηκαν διπλότυπα. 🎉", "No duplicates found. 🎉", "Keine Duplikate gefunden. 🎉", "No se encontraron duplicados. 🎉", "Nessun duplicato trovato. 🎉", "Aucun doublon trouvé. 🎉", "没有找到重复文件。🎉"],
        UninstallSub => ["Αφαιρεί την εφαρμογή μαζί με ρυθμίσεις, cache και υπολείμματα στη Library.", "Removes the app together with its settings, caches and leftovers in Library.", "Entfernt die App samt Einstellungen, Caches und Resten in der Library.", "Elimina la app junto con sus ajustes, cachés y restos en Library.", "Rimuove l'app insieme a impostazioni, cache e residui in Library.", "Supprime l'application avec ses réglages, caches et restes dans Library.", "连同设置、缓存和 Library 中的残留文件一并删除应用。"],
        ReadingApps => ["Ανάγνωση εφαρμογών", "Reading apps", "Apps werden gelesen", "Leyendo aplicaciones", "Lettura delle app", "Lecture des applications", "正在读取应用"],
        SearchHint => ["🔍 Αναζήτηση…", "🔍 Search…", "🔍 Suchen…", "🔍 Buscar…", "🔍 Cerca…", "🔍 Rechercher…", "🔍 搜索…"],
        PickApp => ["← Διάλεξε εφαρμογή", "← Pick an app", "← Wähle eine App", "← Elige una aplicación", "← Scegli un'app", "← Choisis une application", "← 选择一个应用"],
        Version => ["έκδοση {}", "version {}", "Version {}", "versión {}", "versione {}", "version {}", "版本 {}"],
        AppLabel => ["Εφαρμογή:", "App:", "App:", "App:", "App:", "Application :", "应用："],
        LeftoversLabel => ["  Κατάλοιπα:", "  Leftovers:", "  Reste:", "  Restos:", "  Residui:", "  Restes :", "  残留："],
        RelatedFiles => ["Σχετικά αρχεία στη Library", "Related files in Library", "Zugehörige Dateien in der Library", "Archivos relacionados en Library", "File correlati in Library", "Fichiers associés dans Library", "Library 中的相关文件"],
        NotFoundSearching => ["Δεν βρέθηκαν (ή αναζητούνται…)", "None found (or still searching…)", "Keine gefunden (oder Suche läuft…)", "No se encontraron (o aún buscando…)", "Nessuno trovato (o ricerca in corso…)", "Aucun trouvé (ou recherche en cours…)", "未找到（或仍在搜索…）"],
        LensTitle => ["Χάρτης Χώρου", "Space Lens", "Speicher-Lupe", "Lupa de espacio", "Lente dello spazio", "Loupe d'espace", "空间透镜"],
        LensSub => ["Δες ποιοι φάκελοι πιάνουν τον περισσότερο χώρο. Κλικ σε φάκελο για να μπεις.", "See which folders take up the most space. Click a folder to open it.", "Sieh, welche Ordner am meisten Speicher belegen. Klicke auf einen Ordner, um ihn zu öffnen.", "Mira qué carpetas ocupan más espacio. Haz clic en una carpeta para abrirla.", "Vedi quali cartelle occupano più spazio. Clicca su una cartella per aprirla.", "Vois quels dossiers prennent le plus de place. Clique sur un dossier pour l'ouvrir.", "查看哪些文件夹占用空间最多，点击文件夹即可进入。"],
        Up => ["⬆ Πάνω", "⬆ Up", "⬆ Hoch", "⬆ Arriba", "⬆ Su", "⬆ Monter", "⬆ 上级"],
        Home => ["🏠 Αρχικός", "🏠 Home", "🏠 Persönlich", "🏠 Inicio", "🏠 Home", "🏠 Accueil", "🏠 主目录"],
        Applications => ["Εφαρμογές", "Applications", "Programme", "Aplicaciones", "App", "Applications", "应用程序"],
        Library => ["Library", "Library", "Library", "Library", "Library", "Library", "Library"],
        Refresh => ["Ανανέωση", "Refresh", "Aktualisieren", "Actualizar", "Aggiorna", "Actualiser", "刷新"],
        Measuring => ["Μέτρηση φακέλων", "Measuring folders", "Ordner werden gemessen", "Midiendo carpetas", "Misura delle cartelle", "Mesure des dossiers", "正在计算文件夹大小"],
        Total => ["Σύνολο: {}", "Total: {}", "Gesamt: {}", "Total: {}", "Totale: {}", "Total : {}", "总计：{}"],
        MaintSub => ["Εργασίες που κρατούν το macOS γρήγορο. Όσες έχουν 🔒 θα ζητήσουν τον κωδικό σου.", "Tasks that keep macOS fast. Those marked 🔒 will ask for your password.", "Aufgaben, die macOS schnell halten. Bei Einträgen mit 🔒 wird nach deinem Passwort gefragt.", "Tareas que mantienen macOS rápido. Las marcadas con 🔒 pedirán tu contraseña.", "Operazioni che mantengono macOS veloce. Quelle con 🔒 chiederanno la password.", "Tâches qui gardent macOS rapide. Celles marquées 🔒 demanderont ton mot de passe.", "保持 macOS 流畅的任务。标有 🔒 的任务会要求输入密码。"],
        Run => ["Εκτέλεση", "Run", "Ausführen", "Ejecutar", "Esegui", "Exécuter", "运行"],
        MRamName => ["Ελευθέρωση μνήμης RAM", "Free up RAM", "RAM freigeben", "Liberar RAM", "Libera RAM", "Libérer la RAM", "释放内存"],
        MRamDesc => ["Αδειάζει τη μνήμη cache του δίσκου (purge).", "Clears the disk cache memory (purge).", "Leert den Festplatten-Cache-Speicher (purge).", "Vacía la caché de disco en memoria (purge).", "Svuota la cache del disco in memoria (purge).", "Vide le cache disque en mémoire (purge).", "清空磁盘缓存（purge）。"],
        MDnsName => ["Καθαρισμός cache DNS", "Flush DNS cache", "DNS-Cache leeren", "Vaciar caché DNS", "Svuota cache DNS", "Vider le cache DNS", "清除 DNS 缓存"],
        MDnsDesc => ["Βοηθά όταν σελίδες δεν ανοίγουν ή φορτώνουν λάθος διευθύνσεις.", "Helps when pages don't open or load the wrong addresses.", "Hilft, wenn Seiten nicht öffnen oder falsche Adressen laden.", "Útil cuando las páginas no abren o cargan direcciones incorrectas.", "Utile quando le pagine non si aprono o caricano indirizzi errati.", "Utile lorsque les pages ne s'ouvrent pas ou chargent de mauvaises adresses.", "当网页无法打开或加载错误地址时有帮助。"],
        MSpotName => ["Επανευρετηρίαση Spotlight", "Reindex Spotlight", "Spotlight neu indizieren", "Reindexar Spotlight", "Reindicizza Spotlight", "Réindexer Spotlight", "重建 Spotlight 索引"],
        MSpotDesc => ["Ξαναχτίζει το ευρετήριο αναζήτησης. Κάνει μερικά λεπτά στο παρασκήνιο.", "Rebuilds the search index. Takes a few minutes in the background.", "Baut den Suchindex neu auf. Dauert einige Minuten im Hintergrund.", "Reconstruye el índice de búsqueda. Tarda unos minutos en segundo plano.", "Ricostruisce l'indice di ricerca. Richiede alcuni minuti in background.", "Reconstruit l'index de recherche. Prend quelques minutes en arrière-plan.", "重建搜索索引，会在后台运行几分钟。"],
        MQLName => ["Επαναφορά Quick Look", "Reset Quick Look", "Quick Look zurücksetzen", "Restablecer Vista rápida", "Ripristina Quick Look", "Réinitialiser Coup d'œil", "重置快速查看"],
        MQLDesc => ["Διορθώνει προεπισκοπήσεις αρχείων που δεν εμφανίζονται σωστά.", "Fixes file previews that don't display correctly.", "Behebt Dateivorschauen, die nicht richtig angezeigt werden.", "Corrige las vistas previas de archivos que no se muestran bien.", "Corregge le anteprime dei file che non vengono visualizzate correttamente.", "Corrige les aperçus de fichiers qui ne s'affichent pas correctement.", "修复无法正确显示的文件预览。"],
        MLsName => ["Επανακατασκευή Launch Services", "Rebuild Launch Services", "Launch Services neu aufbauen", "Reconstruir Launch Services", "Ricostruisci Launch Services", "Reconstruire Launch Services", "重建 Launch Services"],
        MLsDesc => ["Διορθώνει διπλές εγγραφές στο μενού «Άνοιγμα με».", "Fixes duplicate entries in the «Open With» menu.", "Behebt doppelte Einträge im Menü „Öffnen mit“.", "Corrige entradas duplicadas en el menú «Abrir con».", "Corregge le voci duplicate nel menu «Apri con».", "Corrige les doublons dans le menu « Ouvrir avec ».", "修复“打开方式”菜单中的重复项。"],
        MDone => ["✔ {}: ολοκληρώθηκε", "✔ {}: done", "✔ {}: erledigt", "✔ {}: completado", "✔ {}: completato", "✔ {} : terminé", "✔ {}：已完成"],
        MCancel => ["✖ {}: ακυρώθηκε", "✖ {}: cancelled", "✖ {}: abgebrochen", "✖ {}: cancelado", "✖ {}: annullato", "✖ {} : annulé", "✖ {}：已取消"],
        CatCache => ["Cache εφαρμογών & browsers", "App & browser caches", "App- & Browser-Caches", "Cachés de apps y navegadores", "Cache di app e browser", "Caches des apps et navigateurs", "应用与浏览器缓存"],
        CatCacheDesc => ["Προσωρινά δεδομένα που οι εφαρμογές ξαναδημιουργούν αυτόματα.", "Temporary data that apps recreate automatically.", "Temporäre Daten, die Apps automatisch neu erstellen.", "Datos temporales que las apps vuelven a crear automáticamente.", "Dati temporanei che le app ricreano automaticamente.", "Données temporaires que les applications recréent automatiquement.", "应用会自动重新生成的临时数据。"],
        CatLogs => ["Αρχεία καταγραφής & αναφορές σφαλμάτων", "Logs & crash reports", "Protokolle & Absturzberichte", "Registros e informes de errores", "Log e report di errore", "Journaux et rapports d'erreur", "日志与崩溃报告"],
        CatLogsDesc => ["Logs και diagnostic reports που δεν χρειάζονται για την κανονική λειτουργία.", "Logs and diagnostic reports not needed for normal operation.", "Protokolle und Diagnoseberichte, die für den normalen Betrieb nicht nötig sind.", "Registros e informes de diagnóstico que no son necesarios para el funcionamiento normal.", "Log e report diagnostici non necessari al normale funzionamento.", "Journaux et rapports de diagnostic inutiles au fonctionnement normal.", "正常运行不需要的日志和诊断报告。"],
        CatState => ["Αποθηκευμένη κατάσταση εφαρμογών", "Saved application state", "Gespeicherter App-Zustand", "Estado guardado de las apps", "Stato salvato delle app", "État enregistré des applications", "已保存的应用状态"],
        CatStateDesc => ["Στιγμιότυπα παραθύρων για επαναφορά. Ασφαλή στη διαγραφή.", "Window snapshots used for restoring. Safe to delete.", "Fensterschnappschüsse zur Wiederherstellung. Sicher zu löschen.", "Instantáneas de ventanas para restaurar. Seguro de borrar.", "Istantanee delle finestre per il ripristino. Sicuro da eliminare.", "Instantanés des fenêtres pour la restauration. Supprimable sans risque.", "用于恢复窗口的快照，可安全删除。"],
        CatXcode => ["Xcode & εργαλεία προγραμματισμού", "Xcode & developer tools", "Xcode & Entwicklertools", "Xcode y herramientas de desarrollo", "Xcode e strumenti per sviluppatori", "Xcode et outils de développement", "Xcode 与开发工具"],
        CatXcodeDesc => ["DerivedData, Device Support, cache του simulator, npm, Gradle, Cargo.", "DerivedData, Device Support, simulator cache, npm, Gradle, Cargo.", "DerivedData, Geräteunterstützung, Simulator-Cache, npm, Gradle, Cargo.", "DerivedData, Device Support, caché del simulador, npm, Gradle, Cargo.", "DerivedData, Device Support, cache del simulatore, npm, Gradle, Cargo.", "DerivedData, Device Support, cache du simulateur, npm, Gradle, Cargo.", "DerivedData、设备支持文件、模拟器缓存、npm、Gradle、Cargo。"],
        CatMail => ["Συνημμένα Mail", "Mail attachments", "Mail-Anhänge", "Adjuntos de Mail", "Allegati di Mail", "Pièces jointes de Mail", "邮件附件"],
        CatMailDesc => ["Αντίγραφα συνημμένων που άνοιξες από το Mail. Τα πρωτότυπα μένουν στα email.", "Copies of attachments you opened in Mail. The originals stay in your emails.", "Kopien von Anhängen, die du in Mail geöffnet hast. Die Originale bleiben in deinen E-Mails.", "Copias de los adjuntos que abriste en Mail. Los originales siguen en tus correos.", "Copie degli allegati aperti in Mail. Gli originali restano nelle tue email.", "Copies des pièces jointes ouvertes dans Mail. Les originaux restent dans tes e-mails.", "你在“邮件”中打开过的附件副本。原件仍保留在邮件中。"],
        CatInstallers => ["Εγκαταστάτες στις Λήψεις", "Installers in Downloads", "Installer in den Downloads", "Instaladores en Descargas", "Installer in Download", "Installeurs dans Téléchargements", "“下载”中的安装包"],
        CatInstallersDesc => ["Αρχεία .dmg / .pkg στις Λήψεις. Πάνε στον Κάδο (ανακτήσιμα).", ".dmg / .pkg files in Downloads. Moved to Trash (recoverable).", ".dmg- / .pkg-Dateien in den Downloads. Werden in den Papierkorb verschoben (wiederherstellbar).", "Archivos .dmg / .pkg en Descargas. Se mueven a la papelera (recuperables).", "File .dmg / .pkg in Download. Spostati nel Cestino (recuperabili).", "Fichiers .dmg / .pkg dans Téléchargements. Déplacés dans la Corbeille (récupérables).", "“下载”中的 .dmg / .pkg 文件。将移至废纸篓（可恢复）。"],
        CatBackups => ["Αντίγραφα ασφαλείας iPhone / iPad", "iPhone / iPad backups", "iPhone-/iPad-Backups", "Copias de seguridad de iPhone / iPad", "Backup di iPhone / iPad", "Sauvegardes d'iPhone / iPad", "iPhone / iPad 备份"],
        CatBackupsDesc => ["Τοπικά backups συσκευών iOS. Έλεγξε πριν τα σβήσεις! Πάνε στον Κάδο.", "Local iOS device backups. Check before deleting! Moved to Trash.", "Lokale iOS-Gerätesicherungen. Vorher prüfen! Werden in den Papierkorb verschoben.", "Copias locales de dispositivos iOS. ¡Revísalas antes de borrar! Se mueven a la papelera.", "Backup locali dei dispositivi iOS. Controlla prima di eliminarli! Spostati nel Cestino.", "Sauvegardes locales d'appareils iOS. Vérifie avant de supprimer ! Déplacées dans la Corbeille.", "本地 iOS 设备备份。删除前请先确认！将移至废纸篓。"],
        CatTrash => ["Κάδος απορριμμάτων", "Trash", "Papierkorb", "Papelera", "Cestino", "Corbeille", "废纸篓"],
        CatTrashDesc => ["Αρχεία που ήδη έχεις πετάξει. Η διαγραφή είναι οριστική.", "Files you've already thrown away. Emptying is permanent.", "Dateien, die du bereits weggeworfen hast. Das Leeren ist endgültig.", "Archivos que ya has tirado. Vaciar la papelera es permanente.", "File che hai già gettato. Lo svuotamento è definitivo.", "Fichiers que tu as déjà jetés. Vider la corbeille est définitif.", "你已经丢弃的文件。清空后无法恢复。"],
    }
}
