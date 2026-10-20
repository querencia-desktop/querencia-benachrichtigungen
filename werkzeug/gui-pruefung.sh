#!/bin/bash
# Isolierte GUI-Prüfung von Querencia Benachrichtigungen (G2-L). Nur
# Wegwerfprofil, eigener headless labwc 0.9.8, eigener D-Bus
# (dbus-run-session); die laufende Sitzung bleibt unberührt.
#   bash werkzeug/gui-pruefung.sh [szenario.sh] [debug|release]
# QB_BIN=<binary> prüft eine andere Binary (z. B. die Vorlage der Integration
# mit share/locale daneben); QB_LANGUAGE=en die englische Oberfläche
# (Standard wie die Sitzung: LANG=en_US.UTF-8, LANGUAGE=de); QB_GROESSE
# (BREITExHÖHE) setzt die Auflösung für szenario-sprache.sh und
# szenario-zentrale.sh. Ab 0.2.0: QB_THEMA=1 kopiert GTK-Thema
# Querencia-Klassisch und Symbolthema Querencia (nur gelesen) ins Profil und
# setzt sie dort (wie live); QB_EIGENES_HOME=1 gibt dem Lauf ein eigenes HOME
# im Profil.
set -euo pipefail
E=$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd)
PROFIL=$E/gui-test/profil
# chmod: kopierte Themen aus unveränderlichen Ablagen sind schreibgeschützt.
[ -d "$E/gui-test" ] && [ "$(dirname "$PROFIL")" = "$E/gui-test" ] && { [ ! -d "$PROFIL" ] || chmod -R u+w "$PROFIL"; } && rm -rf "$PROFIL"
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
if [ "${QB_THEMA:-}" = 1 ]; then
    mkdir -p "$PROFIL/data/themes" "$PROFIL/data/icons" "$PROFIL/config/gtk-4.0"
    cp -rL "$HOME/.local/share/themes/Querencia-Klassisch" "$PROFIL/data/themes/"
    cp -rL "$HOME/.local/share/icons/Querencia" "$PROFIL/data/icons/"
    chmod -R u+w "$PROFIL/data/themes" "$PROFIL/data/icons"
    SCHRIFT=$(gsettings get org.gnome.desktop.interface font-name 2>/dev/null | tr -d "'")
    printf '[Settings]\ngtk-theme-name=Querencia-Klassisch\ngtk-icon-theme-name=Querencia\ngtk-font-name=%s\n' \
        "${SCHRIFT:-Ubuntu Sans 11}" >"$PROFIL/config/gtk-4.0/settings.ini"
    export GTK_THEME=Querencia-Klassisch
fi
if [ "${QB_EIGENES_HOME:-}" = 1 ]; then
    mkdir -p "$PROFIL/home"
    export HMP04_HOME=$PROFIL/home
fi
cat >"$PROFIL/env.sh" <<X
BIN=$BIN
ATTRAPPE=$ATTRAPPE
Q=$E/werkzeug/bin/qeingabe
QA=$E/werkzeug/bin/qausgabe
OUT=$E/nachweis/gui
QB_GROESSE=${QB_GROESSE:-}
QB_ALT=${QB_ALT:-}
X
SZ=${1:-szenario.sh}
LANG=en_US.UTF-8 LANGUAGE=${QB_LANGUAGE:-de} \
    python3 -I "$E/werkzeug/isoliert.py" "$PROFIL" "$E/werkzeug/$SZ" "${QB_ZEITLIMIT:-420}"
cp "$PROFIL"/logs/*.log "$E/nachweis/gui/" 2>/dev/null || true
cat "$E/nachweis/gui/ergebnis.txt"
