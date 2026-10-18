#!/bin/bash
# Kurzdiagnose: Was fragt notify-send -A beim Dienst ab? (dbus-monitor)
set -u
. "$HMP04_PROFILE/env.sh"
mkdir -p "$OUT"
R=$OUT/caps-ergebnis.txt; : >"$R"
dbus-monitor >"$OUT/caps-monitor.log" 2>&1 &
MON=$!
sleep 0.5
"$BIN" >"$OUT/caps-dienst.log" 2>&1 &
DIENST=$!
sleep 2
notify-send -A "ja=Ja" "Aktion" "Text" -t 3000 >"$OUT/caps-notify.txt" 2>&1 &
NS=$!
sleep 1
busctl --user call org.freedesktop.Notifications /org/freedesktop/Notifications org.freedesktop.Notifications GetCapabilities >>"$OUT/caps-notify.txt" 2>&1
sleep 3
wait $NS 2>/dev/null
kill $DIENST $MON 2>/dev/null
echo fertig >"$R"
touch "$HMP04_PROFILE/logs/FERTIG"
