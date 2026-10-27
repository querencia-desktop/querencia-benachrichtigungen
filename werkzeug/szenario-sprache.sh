#!/bin/bash
# Läuft IN der isolierten labwc-Sitzung (isoliert.py, eigener D-Bus). Sprache
# von Querencia Benachrichtigungen 0.1.2 (G2-AH) in LANGUAGE (gui-pruefung.sh
# QB_LANGUAGE): eingeblendeter Katalog (/proc/<pid>/maps), Popup mit Tooltip
# am Schließen-Knopf (Bild), Schließen per Klick (NotificationClosed Grund 2),
# kritische Meldung mit Aktionsknöpfen (Bild). Auflösung aus QB_GROESSE
# (Standard 1024x768), Panel-Attrappe 32 px. Nie der Sitzungsbus.
set -u
. "$HMP04_PROFILE/env.sh"
mkdir -p "$OUT"
R=$OUT/ergebnis.txt; : >"$R"
ok() { echo "PASS $*" | tee -a "$R"; }
nok() { echo "FAIL $*" | tee -a "$R"; }
. "$(dirname "$0")/eingabe.sh"
SPRACHE=${LANGUAGE%%:*}
SPRACHE=${SPRACHE%%_*}
GROESSE=${QB_GROESSE:-1024x768}
AUSW=${GROESSE%x*}; AUSH=${GROESSE#*x}
shot() { sleep "${2:-0.6}"; grim "$OUT/$1-$SPRACHE-$GROESSE.png"; }
# qeingabe rechnet Zeigerkoordinaten im 1280x720-Raster (auf den Ausgang skaliert).
zeiger() { q "move $(($1 * 1280 / AUSW)) $(($2 * 720 / AUSH))"; }
zustand() { "$BIN" --nicht-stoeren status 2>&1; }
sichtbar() { zustand | sed -n 's/^Sichtbare Popups: //p'; }
LOG=$OUT/dienst-$SPRACHE.log
echo "INFO LANG=$LANG LANGUAGE=$LANGUAGE Ausgang $GROESSE" >>"$R"
dbus-monitor "type='signal',interface='org.freedesktop.Notifications'" >"$OUT/signale-$SPRACHE.log" 2>&1 &
MON=$!
"$QA" "$GROESSE" 2>>"$OUT/qausgabe.log"; sleep 1
"$ATTRAPPE" 32 >"$OUT/attrappe.log" 2>&1 &
ATTR=$!
sleep 1.5

"$BIN" >"$LOG" 2>&1 &
DIENST=$!
sleep 2
grep -q "bereit: org.freedesktop.Notifications" "$LOG" && ok "Dienst bereit" || nok "Dienst-Start (siehe $LOG)"
KAT=$(grep -o '[^ ]*querencia-benachrichtigungen.mo' "/proc/$DIENST/maps" 2>/dev/null | head -1)
echo "INFO Katalog: ${KAT:-keiner}" >>"$R"
case $SPRACHE in
de | it | es | fr) [ -n "$KAT" ] && ok "$SPRACHE: Katalog eingeblendet" || nok "$SPRACHE: kein Katalog eingeblendet" ;;
*) [ -z "$KAT" ] && ok "$SPRACHE: kein Katalog eingeblendet" || nok "$SPRACHE: Katalog eingeblendet" ;;
esac

# Normale Meldung ohne Ablauf; Zeiger auf den Schließen-Knopf (rechts oben,
# 26 px vom rechten Rand, 59 px von oben unter dem 32-px-Panel) → Tooltip
ID=$(notify-send -p -t 0 "Sprache $SPRACHE" "Tooltip am Schließen-Knopf" 2>&1)
sleep 0.8
zeiger $((AUSW - 40)) 120; q "wait 300"
zeiger $((AUSW - 26)) 59; q "wait 1800"
shot t1-tooltip 0.2
[ "$(sichtbar)" = 1 ] && ok "Meldung $ID sichtbar, Tooltip (Bild t1)" || nok "sichtbar=$(sichtbar)"
q "click" "wait 900"
if [ "$(sichtbar)" = 0 ] && grep -A2 "member=NotificationClosed" "$OUT/signale-$SPRACHE.log" | grep -q "uint32 2"; then
    ok "Klick auf Schließen: Meldung zu, NotificationClosed Grund 2"
else
    nok "Schließen: sichtbar=$(sichtbar)"
fi

# Kritische Meldung mit Aktionsknöpfen (Beschriftung vom Absender)
python3 -I "$(dirname "$0")/melden.py" -u critical -a "ok=OK" -a "spaeter=Later" "Kritisch $SPRACHE" "mit zwei Aktionen" >/dev/null 2>&1 &
MELDER=$!
sleep 1.2
zeiger 200 $((AUSH - 100)); q "wait 300"
shot t2-kritisch
[ "$(sichtbar)" = 1 ] && ok "kritische Meldung mit Aktionen sichtbar (Bild t2)" || nok "kritisch: sichtbar=$(sichtbar)"
kill "$MELDER" 2>/dev/null

kill -0 "$DIENST" 2>/dev/null && ok "Dienst läuft weiter" || nok "Dienst beendet"
kill "$DIENST" "$ATTR" "$MON" 2>/dev/null
echo "Ergebnis $GROESSE $SPRACHE: $(grep -c '^PASS' "$R") PASS, $(grep -c '^FAIL' "$R") FAIL" | tee -a "$R"
touch "$HMP04_PROFILE/logs/FERTIG"
