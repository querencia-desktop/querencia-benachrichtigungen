#!/bin/bash
# Kurzdiagnose Aktionsknopf: Meldung mit zwei Knöpfen, Klick auf „Nein“.
set -u
. "$HMP04_PROFILE/env.sh"
mkdir -p "$OUT"
. "$(dirname "$0")/eingabe.sh"
"$QA" 1024x768 2>/dev/null; sleep 1
"$ATTRAPPE" 32 >/dev/null 2>&1 &
sleep 1
QB_DEBUG=1 "$BIN" >"$OUT/aktion-dienst.log" 2>&1 &
sleep 2
python3 -I "$(dirname "$0")/melden.py" --wait -a "ja=Ja, bitte" -a "nein=Nein" -t 0 "Mit Aktionen" "Klick auf Nein" >"$OUT/aktion-diag.txt" 2>&1 &
NS=$!
sleep 1
grim "$OUT/aktion-diag-vorher.png"
q "move 978 129" "wait 500" "click" "wait 800"
grim "$OUT/aktion-diag-nachher.png"
sleep 1
kill $NS 2>/dev/null
cat "$OUT/aktion-diag.txt"
grep -v "Gdk-WARNING\|libEGL\|^$" "$OUT/aktion-dienst.log"
touch "$HMP04_PROFILE/logs/FERTIG"
