#!/bin/bash
# Isolierte GUI-Prüfung von Querencia Benachrichtigungen (G2-L). Nur
# Wegwerfprofil, eigener headless labwc 0.9.8, eigener D-Bus
# (dbus-run-session); die laufende Sitzung bleibt unberührt.
#   bash werkzeug/gui-pruefung.sh [szenario.sh] [debug|release]
# QB_BIN=<binary> prüft eine andere Binary (z. B. die Vorlage der Integration
# mit share/locale daneben); QB_LANGUAGE=en die englische Oberfläche
# (Standard wie die Sitzung: LANG=en_US.UTF-8, LANGUAGE=de); QB_GROESSE
# (BREITExHÖHE) setzt die Auflösung für szenario-sprache.sh.
set -euo pipefail
E=$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd)
PROFIL=$E/gui-test/profil
[ -d "$E/gui-test" ] && [ "$(dirname "$PROFIL")" = "$E/gui-test" ] && rm -rf "$PROFIL"
mkdir -p "$PROFIL/data/applications" "$E/nachweis/gui" "$E/laufzeit"
ART=${2:-debug}
BIN=${QB_BIN:-$E/target/$ART/querencia-benachrichtigungen}
ATTRAPPE=$E/target/$ART/panel-attrappe
[ -x "$BIN" ] || { echo "fehlt: $BIN (erst cargo build)"; exit 2; }
[ -x "$ATTRAPPE" ] || { echo "fehlt: $ATTRAPPE (erst cargo build --all-targets)"; exit 2; }
# .desktop-Eintrag für den Hint desktop-entry (Symbol aus der Datei)
cat >"$PROFIL/data/applications/org.test.Probe.desktop" <<X
[Desktop Entry]
Type=Application
Name=Probe
Icon=mail-unread
Exec=true
X
cat >"$PROFIL/env.sh" <<X
BIN=$BIN
ATTRAPPE=$ATTRAPPE
Q=$E/werkzeug/bin/qeingabe
QA=$E/werkzeug/bin/qausgabe
OUT=$E/nachweis/gui
QB_GROESSE=${QB_GROESSE:-}
X
SZ=${1:-szenario.sh}
LANG=en_US.UTF-8 LANGUAGE=${QB_LANGUAGE:-de} \
    python3 -I "$E/werkzeug/isoliert.py" "$PROFIL" "$E/werkzeug/$SZ" "${QB_ZEITLIMIT:-420}"
cp "$PROFIL"/logs/*.log "$E/nachweis/gui/" 2>/dev/null || true
cat "$E/nachweis/gui/ergebnis.txt"
