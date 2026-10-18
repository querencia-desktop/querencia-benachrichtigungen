#!/bin/bash
# Übergroße Eingaben (Fehlerbericht 0.1.1): image-data 512×512, app_icon-Pfad
# auf großes PNG, 2000-Zeichen-Body, 300-Zeichen-Titel ohne Leerzeichen,
# bei 1920×1080 unter einer 32-px-Panel-Attrappe. Misst die Kastenhöhen.
#   QB_BIN=<binary> bash werkzeug/gui-pruefung.sh szenario-gross.sh
#   Ausgabe: $OUT/gross-<kennung>-*.png, $OUT/gross-ergebnis-<kennung>.txt
set -u
. "$HMP04_PROFILE/env.sh"
mkdir -p "$OUT"
K=${QB_KENNUNG:-$(basename "$(dirname "$(dirname "$BIN")")")}
R=$OUT/gross-ergebnis-$K.txt; : >"$R"
ok() { echo "PASS $*" | tee -a "$R"; }
nok() { echo "FAIL $*" | tee -a "$R"; }
BILD=$(dirname "$0")/bild.py
MELDEN="python3 -I $(dirname "$0")/melden.py"
"$QA" 1920x1080 2>/dev/null; sleep 1
"$ATTRAPPE" 32 >/dev/null 2>&1 &
sleep 1
"$BIN" >"$OUT/gross-dienst-$K.log" 2>&1 &
DIENST=$!
sleep 2
echo "Binary: $BIN ($("$BIN" --version))" | tee -a "$R"
# Kastenhöhe: helle Zeilen in Spalte w-30 ab y=32 zählen (Kasten ist hell, Hintergrund schwarz)
masse() { # Bild -> "erste_helle_zeile letzte_helle_zeile linke_kante"
    local w h y0 y1 xl
    read -r w h < <(python3 -I "$BILD" "$1" groesse)
    y0=$(python3 -I "$BILD" "$1" erste-helle-zeile $((w-30)))
    y1=$(python3 -I "$BILD" "$1" letzte-helle-zeile $((w-30)))
    xl=$(python3 -I "$BILD" "$1" linke-kante $((y0+4)))
    echo "$y0 $y1 $xl"
}
pruefe() { # kennung bild
    local y0 y1 xl hoehe breite
    read -r y0 y1 xl < <(masse "$2")
    hoehe=$((y1 - y0 + 1)); breite=$((1920 - 8 - xl))
    echo "$1: Kasten oben $y0, unten $y1, Höhe $hoehe, Breite $breite" | tee -a "$R"
    if [ "$hoehe" -le 170 ] && [ "$breite" -ge 355 ] && [ "$breite" -le 370 ]; then ok "$1: Höhe $hoehe ≤ 170, Breite $breite ≈ 360"; else nok "$1: Höhe $hoehe, Breite $breite"; fi
}
schliesse_alle() { local i; for i in "$@"; do gdbus call --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications --method org.freedesktop.Notifications.CloseNotification "$i" >/dev/null 2>&1; done; sleep 0.6; }

# 1 image-data 512×512
ID=$($MELDEN --bild 512x512 -t 0 "Bild 512×512 als image-data" "Wie Electron/libnotify: Programmsymbol als Pixelbild")
sleep 1.2; grim "$OUT/gross-$K-1-image-data-512.png"; pruefe "image-data 512x512" "$OUT/gross-$K-1-image-data-512.png"; schliesse_alle "$ID"
# 2 app_icon als Pfad auf großes PNG (256×256)
ID=$($MELDEN -i /usr/share/icons/mate/256x256/status/dialog-information.png -t 0 "app_icon-Pfad 256×256" "PNG-Datei als app_icon")
sleep 1.2; grim "$OUT/gross-$K-2-app-icon-256.png"; pruefe "app_icon 256x256" "$OUT/gross-$K-2-app-icon-256.png"; schliesse_alle "$ID"
# 3 Body 2000 Zeichen
BODY=$(python3 -I -c "print(' '.join(f'Wort{i}' for i in range(1, 330))[:2000])")
ID=$($MELDEN -t 0 "Body mit 2000 Zeichen" "$BODY")
sleep 1.2; grim "$OUT/gross-$K-3-body-2000.png"; pruefe "Body 2000" "$OUT/gross-$K-3-body-2000.png"; schliesse_alle "$ID"
# 4 Titel 300 Zeichen ohne Leerzeichen
TITEL=$(python3 -I -c "print(('Titelohneleerzeichen' * 15)[:300])")
ID=$($MELDEN -t 0 "$TITEL" "Titel mit 300 Zeichen ohne Leerzeichen")
sleep 1.2; grim "$OUT/gross-$K-4-titel-300.png"; pruefe "Titel 300" "$OUT/gross-$K-4-titel-300.png"; schliesse_alle "$ID"
# 5 alles zusammen mit Aktionen
ID=$($MELDEN --bild 1024x256 -a "ja=Ja" -a "nein=Nein" -t 0 "$TITEL" "$BODY")
sleep 1.2; grim "$OUT/gross-$K-5-alles.png"; pruefe "alles + Aktionen" "$OUT/gross-$K-5-alles.png"; schliesse_alle "$ID"
kill -TERM $DIENST 2>/dev/null
grep -i "critical\|warning" "$OUT/gross-dienst-$K.log" | grep -v "vkGet\|libEGL" | head -5
echo "Ergebnis: $(grep -c '^PASS' "$R") PASS, $(grep -c '^FAIL' "$R") FAIL" | tee -a "$R"
touch "$HMP04_PROFILE/logs/FERTIG"
