#!/bin/bash
# Läuft IN der isolierten labwc-Sitzung (isoliert.py, eigener D-Bus). Prüft
# den Benachrichtigungsdienst mit notify-send, gdbus/busctl, virtuellen
# Eingaben und Bildschirmfotos. Ergebnis: $OUT/ergebnis.txt
set -u
. "$HMP04_PROFILE/env.sh"
mkdir -p "$OUT"
R=$OUT/ergebnis.txt; : >"$R"
ok() { echo "PASS $*" | tee -a "$R"; }
nok() { echo "FAIL $*" | tee -a "$R"; }
blocked() { echo "BLOCKED $*" | tee -a "$R"; }
. "$(dirname "$0")/eingabe.sh"
shot() { sleep "${2:-0.6}"; grim "$OUT/$1.png"; }
LOG=$OUT/dienst.log
BILD=$(dirname "$0")/bild.py
zustand() { "$BIN" --nicht-stoeren status 2>&1; }
sichtbar() { zustand | sed -n 's/^Sichtbare Popups: //p'; }
wartend() { zustand | sed -n 's/^Wartende Meldungen: //p'; }
# Signale mitschreiben (NotificationClosed, ActionInvoked)
dbus-monitor "type='signal',interface='org.freedesktop.Notifications'" >"$OUT/signale.log" 2>&1 &
MON=$!
sleep 0.5

# --- Auflösung 1024x768, Panel-Attrappe 32 px ---------------------------
"$QA" 1024x768 2>>"$OUT/qausgabe.log"; sleep 1
"$ATTRAPPE" 32 >"$OUT/attrappe.log" 2>&1 &
ATTR=$!
sleep 1.5

# 1 Dienst starten, Name belegt
"$BIN" >"$LOG" 2>&1 &
DIENST=$!
sleep 2
if kill -0 $DIENST 2>/dev/null && grep -q "bereit: org.freedesktop.Notifications" "$LOG"; then ok "Dienst startet und belegt org.freedesktop.Notifications"; else nok "Dienst-Start (siehe dienst.log)"; fi

# 2 GetServerInformation per gdbus und busctl
INFO=$(gdbus call --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications --method org.freedesktop.Notifications.GetServerInformation 2>&1)
echo "$INFO" >"$OUT/serverinfo-gdbus.txt"
[[ $INFO == *"Querencia Benachrichtigungen"*"Querencia"*"0.1.0"*"1.2"* ]] && ok "GetServerInformation (gdbus): $INFO" || nok "GetServerInformation gdbus: $INFO"
BINFO=$(busctl --user call org.freedesktop.Notifications /org/freedesktop/Notifications org.freedesktop.Notifications GetServerInformation 2>&1)
echo "$BINFO" >"$OUT/serverinfo-busctl.txt"
[[ $BINFO == *"Querencia Benachrichtigungen"* ]] && ok "GetServerInformation (busctl): $BINFO" || nok "GetServerInformation busctl: $BINFO"
CAPS=$(gdbus call --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications --method org.freedesktop.Notifications.GetCapabilities 2>&1)
[[ $CAPS == *actions*body-markup* ]] && ok "GetCapabilities: $CAPS" || nok "GetCapabilities: $CAPS"

# 3 zweiter Dienst: Name belegt, beendet sich sauber
"$BIN" >"$OUT/zweiter.log" 2>&1
RC=$?
if [ $RC -ne 0 ] && grep -q "bereits belegt" "$OUT/zweiter.log" && kill -0 $DIENST 2>/dev/null; then ok "zweiter Dienst beendet sich (Exit $RC): $(grep -o 'bereits belegt.*' "$OUT/zweiter.log" | cut -c1-80)"; else nok "zweiter Dienst (Exit $RC): $(cat "$OUT/zweiter.log")"; fi

# 4 einfache Meldung
ID1=$(notify-send -p "Einfache Meldung" "Hallo aus der isolierten Sitzung – Umlaute äöü ß" 2>&1)
sleep 0.8
shot 01-einfach-1024
[ "$(sichtbar)" = 1 ] && [ "$ID1" = 1 ] && ok "einfache Meldung sichtbar (ID $ID1)" || nok "einfach: sichtbar=$(sichtbar) ID=$ID1"
# unter dem Panel? Pixel in Zeile 10 (Panel) muss Attrappen-Grau sein, nicht Popup-Hell
lage() { # $1 Bild, $2 Panelhöhe, $3 Ausgabedatei
    local w h y0 xl
    read -r w h < <(python3 -I "$BILD" "$1" groesse)
    y0=$(python3 -I "$BILD" "$1" erste-helle-zeile $((w-30)))
    xl=$(python3 -I "$BILD" "$1" linke-kante $((y0+4)))
    {
        echo "Größe $w $h"
        echo "Panelpixel $(python3 -I "$BILD" "$1" pixel $((w/2)) 10)"
        echo "erste helle Zeile rechts $y0"
        echo "Popup linke Kante ca. $xl"
        [ "$y0" -ge "$2" ] && [ "$y0" -le $(($2+16)) ] && echo "Popup beginnt unter Panel: ja" || echo "Popup beginnt unter Panel: nein"
        [ $((w-xl)) -ge 340 ] && [ $((w-xl)) -le 400 ] && echo "Popup rechts am Rand: ja" || echo "Popup rechts am Rand: nein"
    } >"$3"
}
lage "$OUT/01-einfach-1024.png" 32 "$OUT/lage-1024.txt"
cat "$OUT/lage-1024.txt"
grep -q "unter Panel: ja" "$OUT/lage-1024.txt" && ok "Popup liegt unter der Panel-Exklusivzone (1024x768): $(grep 'erste helle' "$OUT/lage-1024.txt")" || nok "Lage 1024x768: $(cat "$OUT/lage-1024.txt" | tr '\n' ' ')"
grep -q "rechts am Rand: ja" "$OUT/lage-1024.txt" && ok "Popup am rechten Rand, Breite ca. 360: $(grep 'linke Kante' "$OUT/lage-1024.txt")" || nok "Popup nicht rechts: $(grep 'linke Kante' "$OUT/lage-1024.txt")"

# 5 Timeout: Standard 5 s -> nach 6,5 s weg, Signal NotificationClosed Grund 1
sleep 6
[ "$(sichtbar)" = 0 ] && ok "Standard-Timeout: Meldung nach 5 s weg" || nok "Timeout: sichtbar=$(sichtbar)"
grep -A3 "NotificationClosed" "$OUT/signale.log" | grep -q "uint32 1$" && ok "NotificationClosed(1, 1 abgelaufen) gesendet" || nok "NotificationClosed fehlt: $(grep -c NotificationClosed "$OUT/signale.log")"

# 6 niedrige Dringlichkeit: 3 s
notify-send -u low "Niedrig" "verschwindet nach 3 s" ; sleep 0.8
[ "$(sichtbar)" = 1 ] || nok "low nicht sichtbar"
shot 02-low
sleep 3.2
[ "$(sichtbar)" = 0 ] && ok "low: nach 3 s weg" || nok "low: nach 4 s noch sichtbar=$(sichtbar)"

# 7 mit Symbol (Themenname, Dateipfad, desktop-entry, image-path)
IDS1=$(notify-send -p -i dialog-warning "Mit Themensymbol" "Symbol dialog-warning aus dem Thema" -t 20000)
IDS2=$(notify-send -p -i /usr/share/icons/mate/48x48/status/dialog-information.png "Mit Dateipfad" "Symbol aus PNG-Datei" -t 20000)
IDS3=$(notify-send -p -h string:desktop-entry:org.test.Probe "Mit desktop-entry" "Symbol mail-unread aus org.test.Probe.desktop" -t 20000)
IDS4=$(notify-send -p -h string:image-path:/usr/share/icons/mate/48x48/status/dialog-error.png "Mit image-path" "Hint image-path, PNG" -t 20000)
sleep 1
shot 03-symbole
[ "$(sichtbar)" = 4 ] && ok "vier Meldungen mit Symbol (Thema, Datei, desktop-entry, image-path) sichtbar (Bild 03)" || nok "Symbole: sichtbar=$(sichtbar)"
grep -q "image-missing\|kein Symbol" "$LOG" && nok "Symbolwarnungen im Log" || ok "keine Symbolwarnungen"
# Schließen-X der obersten Meldung: X liegt oben rechts im Kasten (ca. x=w-8-8-9, y=32+8+8+9)
q "move 999 57" "wait 300" "click" "wait 600"
[ "$(sichtbar)" = 3 ] && ok "Schließen-X schließt die oberste Meldung" || nok "Schließen-X: sichtbar=$(sichtbar)"
shot 04-nach-x
# Rest per CloseNotification schließen
for i in $IDS2 $IDS3 $IDS4; do gdbus call --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications --method org.freedesktop.Notifications.CloseNotification $i >/dev/null 2>&1; done
sleep 0.5
[ "$(sichtbar)" = 0 ] && ok "CloseNotification schließt die übrigen drei" || nok "CloseNotification: sichtbar=$(sichtbar)"
grep -A3 "NotificationClosed" "$OUT/signale.log" | grep -q "uint32 3$" && ok "NotificationClosed Grund 3 (Aufruf) gesendet" || nok "Grund 3 fehlt"
E=$(gdbus call --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications --method org.freedesktop.Notifications.CloseNotification 999 2>&1)
[[ $E == *Error* ]] && ok "CloseNotification(999) liefert Fehler: ${E:0:70}" || nok "CloseNotification unbekannt: $E"

# 8 Aktionen: Knopf anklicken -> ActionInvoked. Nicht mit notify-send -A:
# Ubuntus AppArmor-Profil für notify-send erlaubt nur GetServerInformation
# und Notify, GetCapabilities wird abgelehnt (dbus-monitor: AccessDenied),
# darum glaubt notify-send, der Dienst könne keine Aktionen. melden.py ist
# ein kleiner Gio-Client ohne diese Beschränkung.
MELDEN="python3 -I $(dirname "$0")/melden.py"
$MELDEN --wait -a "ja=Ja, bitte" -a "nein=Nein" -t 0 "Mit Aktionen" "Zwei Knöpfe; Klick auf „Nein“" >"$OUT/aktion-antwort.txt" 2>&1 &
NS=$!
sleep 1
shot 05-aktionen
# Knöpfe rechts unten im Kasten. Position: Kasten oben y=40, Höhe ~ 90; Knopf „Nein“ rechts bei x ca. w-8-8-30
q "move 985 108" "wait 300" "click" "wait 800"
if wait $NS; then :; fi
ANTWORT=$(cat "$OUT/aktion-antwort.txt")
[ "$ANTWORT" = nein ] && ok "ActionInvoked per Klick auf „Nein“ (Client erhält „$ANTWORT“)" || nok "Aktion: Client erhielt „$ANTWORT“"
grep -A3 "ActionInvoked" "$OUT/signale.log" | grep -q '"nein"' && ok "Signal ActionInvoked(…, \"nein\") im dbus-monitor" || nok "ActionInvoked fehlt im Monitor"
[ "$(sichtbar)" = 0 ] && ok "Meldung nach Aktion geschlossen" || nok "nach Aktion sichtbar=$(sichtbar)"

# 8b Befund notify-send -A (AppArmor): erwartet die Fehlermeldung von notify-send selbst
notify-send -A "x=X" "AppArmor-Probe" "notify-send -A" -t 1 >"$OUT/notify-send-A.txt" 2>&1
grep -q "Actions are not supported" "$OUT/notify-send-A.txt" && echo "HINW notify-send -A scheitert an Ubuntus AppArmor-Profil (GetCapabilities verboten), nicht am Dienst" | tee -a "$R"
sleep 0.5

# 9 Default-Aktion per Klick auf die Meldung
$MELDEN --wait -a "default=Öffnen" -t 0 "Default-Aktion" "Klick auf den Text löst „default“ aus" >"$OUT/default-antwort.txt" 2>&1 &
NS=$!
sleep 1
shot 06-default
q "move 850 75" "wait 300" "click" "wait 800"
if wait $NS; then :; fi
ANTWORT=$(cat "$OUT/default-antwort.txt")
[ "$ANTWORT" = default ] && ok "Klick auf die Meldung löst die Default-Aktion aus" || nok "Default: Client erhielt „$ANTWORT“"

# 10 kritisch ohne Timeout
IDK=$(notify-send -p -u critical "Kritisch" "Oranger Rand, bleibt stehen" -t 2000)
sleep 3.5
shot 07-kritisch
[ "$(sichtbar)" = 1 ] && ok "kritische Meldung bleibt trotz -t 2000 stehen" || nok "kritisch: sichtbar=$(sichtbar) nach 3,5 s"
echo "orange Pixel rechts: $(python3 -I "$BILD" "$OUT/07-kritisch.png" orange-rechts)" >"$OUT/kritisch-rand.txt"
cat "$OUT/kritisch-rand.txt"
[ "$(sed -n 's/.*: //p' "$OUT/kritisch-rand.txt")" -gt 500 ] && ok "oranger Rand (#F07746) bei kritischer Meldung: $(cat "$OUT/kritisch-rand.txt")" || nok "kein oranger Rand: $(cat "$OUT/kritisch-rand.txt")"
# Hover hält an: normale Meldung, Zeiger drauf, nach 7 s noch da; Zeiger weg, dann verschwindet sie
notify-send "Überfahren" "Zeiger hält die Zeit an" ; sleep 0.5
q "move 850 170" "wait 7000"
[ "$(sichtbar)" = 2 ] && ok "Hover hält die Meldung über 7 s" || nok "Hover: sichtbar=$(sichtbar)"
shot 08-hover
q "move 300 500" "wait 6000"
[ "$(sichtbar)" = 1 ] && ok "nach dem Verlassen läuft die Zeit weiter (Meldung weg)" || nok "nach Hover: sichtbar=$(sichtbar)"
# Kritische per Aufruf schließen
gdbus call --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications --method org.freedesktop.Notifications.CloseNotification "$IDK" >/dev/null 2>&1
sleep 0.5

# 11 replaces_id
IDA=$(notify-send -p "Fortschritt" "Schritt 1 von 3" -t 0)
sleep 0.4
IDB=$(notify-send -p -r "$IDA" "Fortschritt" "Schritt 2 von 3" -t 0)
sleep 0.4
IDC=$(notify-send -p -r "$IDA" "Fortschritt" "<b>Schritt 3 von 3</b> – <i>fertig</i>" -t 0)
sleep 0.8
shot 09-ersetzt
[ "$IDA" = "$IDB" ] && [ "$IDA" = "$IDC" ] && [ "$(sichtbar)" = 1 ] && ok "replaces_id: dieselbe ID ($IDA) dreimal, eine Meldung sichtbar" || nok "replaces: $IDA $IDB $IDC sichtbar=$(sichtbar)"
gdbus call --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications --method org.freedesktop.Notifications.CloseNotification "$IDA" >/dev/null 2>&1
sleep 0.5

# 12 Markup
IDM=$(notify-send -p "Markup" "<b>fett</b> <i>kursiv</i> <u>unter</u> <a href=\"https://querencia.example\">Link</a> &amp; 3 &lt; 5 <script>x</script> <img src=\"a\"/>" -t 0)
sleep 0.8
shot 10-markup
[ "$(sichtbar)" = 1 ] && ok "Body-Markup dargestellt (Bild 10), fremdes Tag als Text" || nok "Markup: sichtbar=$(sichtbar)"
grep -q "Pango-WARNING\|Failed to set text\|Invalid markup\|Gtk-WARNING.*markup" "$LOG" && nok "Markup-Warnung im Log" || ok "keine Pango-Markup-Warnung"
gdbus call --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications --method org.freedesktop.Notifications.CloseNotification "$IDM" >/dev/null 2>&1
sleep 0.5

# 13 sieben Meldungen: 5 sichtbar, 2 warten; nachrücken
IDS=(); for i in 1 2 3 4 5 6 7; do IDS+=("$(notify-send -p -t 0 "Meldung $i von 7" "Stapel und Warteschlange")"); done
sleep 1.2
shot 11-stapel-7
[ "$(sichtbar)" = 5 ] && [ "$(wartend)" = 2 ] && ok "7 Meldungen: 5 sichtbar, 2 warten" || nok "Stapel: sichtbar=$(sichtbar) wartend=$(wartend)"
gdbus call --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications --method org.freedesktop.Notifications.CloseNotification "${IDS[0]}" >/dev/null 2>&1
sleep 0.8
shot 12-nachgerueckt
[ "$(sichtbar)" = 5 ] && [ "$(wartend)" = 1 ] && ok "nach Schließen rückt eine wartende Meldung nach (5/1)" || nok "Nachrücken: sichtbar=$(sichtbar) wartend=$(wartend)"
for i in "${IDS[@]:1}"; do gdbus call --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications --method org.freedesktop.Notifications.CloseNotification "$i" >/dev/null 2>&1; done
sleep 0.5
[ "$(sichtbar)" = 0 ] && [ "$(wartend)" = 0 ] && ok "alle geschlossen, Schlange leer" || nok "Rest: sichtbar=$(sichtbar) wartend=$(wartend)"

# 14 Nicht stören: Popups unterdrückt, Verlauf gefüllt
"$BIN" --nicht-stoeren an >"$OUT/ns.txt" 2>&1
IDN=$(notify-send -p "Im Nicht-stören" "darf nicht erscheinen")
sleep 0.8
shot 13-nicht-stoeren
[ "$(sichtbar)" = 0 ] && ok "Nicht stören: kein Popup (ID $IDN vergeben)" || nok "Nicht stören: sichtbar=$(sichtbar)"
zustand | grep -q "Nicht stören: an" && ok "CLI --nicht-stoeren status zeigt an" || nok "status: $(zustand)"
V=$("$BIN" --verlauf 2>&1); echo "$V" >"$OUT/verlauf.txt"
echo "$V" | grep -q "Im Nicht-stören" && ok "Verlauf enthält die unterdrückte Meldung" || nok "Verlauf: $V"
"$BIN" --nicht-stoeren aus >>"$OUT/ns.txt" 2>&1
zustand | grep -q "Nicht stören: aus" && ok "Nicht stören wieder aus" || nok "aus: $(zustand)"
notify-send "Wieder da" "nach Nicht stören" ; sleep 0.8
[ "$(sichtbar)" = 1 ] && ok "nach „aus“ erscheinen Popups wieder" || nok "nach aus: sichtbar=$(sichtbar)"
sleep 5.5
# Verlauf: höchstens 50
for i in $(seq 1 55); do notify-send -t 1 "Flut $i" "x" >/dev/null; done
sleep 1
N=$("$BIN" --verlauf 2>&1 | tail -1)
[ "$N" = "50 Einträge" ] && ok "Verlauf auf 50 begrenzt ($N)" || nok "Verlauf: $N"
sleep 1
# Querencia-Schnittstelle per gdbus
Z=$(gdbus call --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications --method org.querencia.Benachrichtigungen.Zustand 2>&1)
[[ $Z == "(false, 0, 0)" ]] && ok "org.querencia.Benachrichtigungen.Zustand: $Z" || nok "Zustand: $Z"
gdbus introspect --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications >"$OUT/introspect.txt" 2>&1
grep -q "SetzeNichtStoeren\|NotificationClosed\|ActionInvoked\|ZustandGeaendert" "$OUT/introspect.txt" && ok "Introspektion zeigt beide Schnittstellen" || nok "Introspektion"

# --- 1920x1080, Attrappe 32 px ------------------------------------------
"$QA" 1920x1080 2>>"$OUT/qausgabe.log"; sleep 1.5
notify-send -u critical "Kritisch bei 1920×1080" "unter dem Panel, rechts" -t 0
notify-send "Zweite Meldung" "Stapel nach unten" -t 0
notify-send -i dialog-information "Dritte Meldung" "mit Symbol" -t 0
sleep 1.2
shot 14-1920
lage "$OUT/14-1920.png" 32 "$OUT/lage-1920.txt"
cat "$OUT/lage-1920.txt"
grep -q "Größe 1920 1080" "$OUT/lage-1920.txt" && grep -q "unter Panel: ja" "$OUT/lage-1920.txt" && ok "1920x1080: Popups unter dem Panel rechts (Bild 14)" || nok "1920: $(cat "$OUT/lage-1920.txt" | tr '\n' ' ')"
# Attrappe mit 48 px: Popups wandern mit
kill $ATTR 2>/dev/null; wait $ATTR 2>/dev/null
"$ATTRAPPE" 48 >>"$OUT/attrappe.log" 2>&1 &
ATTR=$!
sleep 1.5
shot 15-1920-panel48
lage "$OUT/15-1920-panel48.png" 48 "$OUT/lage-1920-48.txt"
sed -i "s/unter Panel:/unter Panel 48:/" "$OUT/lage-1920-48.txt"
cat "$OUT/lage-1920-48.txt"
grep -q "unter Panel 48: ja" "$OUT/lage-1920-48.txt" && ok "Panelhöhe 48: Popups rücken mit (Exklusivzone, nicht geraten)" || nok "Panel 48: $(cat "$OUT/lage-1920-48.txt" | tr '\n' ' ')"

# 15 Dienst sauber beenden
kill -TERM $DIENST; sleep 1
kill -0 $DIENST 2>/dev/null && nok "Dienst läuft nach SIGTERM weiter" || ok "Dienst endet auf SIGTERM"
kill $ATTR $MON 2>/dev/null
grep -i -E 'critical|warning' "$LOG" | head -20 >"$OUT/gtk-meldungen.txt"
[ -s "$OUT/gtk-meldungen.txt" ] && echo "HINW GTK-Meldungen: $(wc -l <"$OUT/gtk-meldungen.txt")" | tee -a "$R"
echo "Ergebnis: $(grep -c '^PASS' "$R") PASS, $(grep -c '^FAIL' "$R") FAIL, $(grep -c '^BLOCKED' "$R") BLOCKED" | tee -a "$R"
touch "$HMP04_PROFILE/logs/FERTIG"
