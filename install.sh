#!/bin/bash
# Katharos — εγκατάσταση σε macOS.
# Χρήση:  bash install.sh
set -e
cd "$(dirname "$0")"
APP="Katharos"

echo ""
echo "✨ Katharos — εγκατάσταση"
echo "─────────────────────────"

if [ "$(uname)" != "Darwin" ]; then
  echo "Αυτό το script τρέχει μόνο σε macOS."; exit 1
fi

# 1. Εργαλεία της Apple (compiler/linker)
if ! xcode-select -p >/dev/null 2>&1; then
  echo "==> Χρειάζονται τα Command Line Tools της Apple."
  echo "    Θα ανοίξει παράθυρο εγκατάστασης. Όταν τελειώσει, τρέξε ξανά:  bash install.sh"
  xcode-select --install || true
  exit 1
fi

# 2. Rust (μόνο την πρώτη φορά)
if ! command -v cargo >/dev/null 2>&1; then
  if [ -x "$HOME/.cargo/bin/cargo" ]; then
    export PATH="$HOME/.cargo/bin:$PATH"
  else
    echo "==> Εγκατάσταση Rust (μία φορά, ~1 λεπτό)…"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
  fi
fi

# 3. Μεταγλώττιση με πλήρεις βελτιστοποιήσεις
echo "==> Μεταγλώττιση (2–5 λεπτά την πρώτη φορά)…"
cargo build --release

# 4. Δημιουργία Katharos.app
echo "==> Δημιουργία εφαρμογής…"
BUILD="build/$APP.app"
rm -rf build
mkdir -p "$BUILD/Contents/MacOS" "$BUILD/Contents/Resources"
cp target/release/katharos "$BUILD/Contents/MacOS/katharos"
cp macos/Info.plist "$BUILD/Contents/Info.plist"

ICONSET="build/AppIcon.iconset"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z $s $s macos/icon.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2))
  sips -z $d $d macos/icon.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$BUILD/Contents/Resources/AppIcon.icns"
codesign --force --deep --sign - "$BUILD" >/dev/null 2>&1 || true

# 5. Αντιγραφή στις Εφαρμογές
DEST="/Applications"
if [ ! -w "$DEST" ]; then
  DEST="$HOME/Applications"
  mkdir -p "$DEST"
fi
rm -rf "${DEST:?}/$APP.app"
cp -R "$BUILD" "$DEST/"

echo ""
# 6. Προαιρετικά: ClamAV για σάρωση ιών
if command -v brew >/dev/null 2>&1 && ! command -v clamscan >/dev/null 2>&1; then
  echo ""
  read -r -p "==> Να εγκατασταθεί το ClamAV για σάρωση ιών; (y/N) " ans
  if [ "$ans" = "y" ] || [ "$ans" = "Y" ]; then
    brew install clamav || echo "    Η εγκατάσταση του ClamAV απέτυχε. Δοκίμασε αργότερα: brew install clamav"
  fi
fi

echo ""
echo "✅ Έτοιμο! Το Katharos εγκαταστάθηκε στο $DEST/$APP.app"
echo "   Το βρίσκεις στο Launchpad / Spotlight (⌘ + Space → Katharos)."
echo ""
echo "   Συμβουλή: για πλήρη καθαρισμό (Mail, Κάδος κ.λπ.) πρόσθεσε το Katharos στο"
echo "   Ρυθμίσεις συστήματος → Απόρρητο & ασφάλεια → Πλήρης πρόσβαση δίσκου."
echo ""
open "$DEST/$APP.app"
