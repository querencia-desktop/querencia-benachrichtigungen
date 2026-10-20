#!/bin/bash
# Läuft IN der isolierten labwc-Sitzung (isoliert.py, eigener D-Bus, eigenes
# HOME und XDG-Profil). Mitteilungszentrale von Querencia Benachrichtigungen
# 0.2.0: Verlauf und Persistenz, Schnittstelle org.querencia.Benachrichtigungen,
# Fenster mit Tasten (Esc, Entf, Eingabe, Tab, Strg+F, Zugriffstasten), Nicht stören
# (kritisch kommt durch), ini (anzeigen, position, dauer_s, hinweis, verlauf),
# Neustart, --zentrale ohne Dienst und mit älterem Dienst (QB_ALT).
# Auflösung QB_GROESSE (Standard 1024x768), Sprache aus LANGUAGE. Bilder
# z<NN>-<name>-<sprache>-<größe>.png. Nie der Sitzungsbus.
set -u
. "$HMP04_PROFILE/env.sh"
mkdir -p "$OUT"
R=$OUT/ergebnis.txt; : >"$R"
ok() { echo "PASS $*" | tee -a "$R"; }
nok() { echo "FAIL $*" | tee -a "$R"; }
info() { echo "INFO $*" | tee -a "$R"; }
pruefe() { local t=$1; shift; if "$@"; then ok "$t"; else nok "$t"; fi; }
. "$(dirname "$0")/eingabe.sh"
BILD=$(dirname "$0")/bild.py
MELDEN=$(dirname "$0")/melden.py
SPRACHE=${LANGUAGE%%:*}
GROESSE=${QB_GROESSE:-1024x768}
AUSW=${GROESSE%x*}; AUSH=${GROESSE#*x}
S=$SPRACHE-$GROESSE
shot() { sleep "${2:-0.7}"; grim "$OUT/$1-$S.png"; }
zeiger() { q "move $(($1 * 1280 / AUSW)) $(($2 * 720 / AUSH))"; }
STATE=$XDG_STATE_HOME/querencia
INI=$XDG_CONFIG_HOME/querencia/benachrichtigungen.ini
JSON=$STATE/benachrichtigungen.json
LOG=$OUT/dienst-zentrale-$S.log
info "LANG=$LANG LANGUAGE=$LANGUAGE Ausgang $GROESSE HOME=$HOME"
info "Thema GTK_THEME=${GTK_THEME:-} settings.ini: $(tr '\n' ' ' <"$XDG_CONFIG_HOME/gtk-4.0/settings.ini" 2>/dev/null)"
[ "$HOME" != "$(getent passwd "$(id -un)" | cut -d: -f6)" ] && ok "eigenes HOME im Profil ($HOME)" || nok "HOME ist das echte Heimatverzeichnis"
case $XDG_RUNTIME_DIR in */qed-runtime-*) ok "eigenes XDG_RUNTIME_DIR" ;; *) nok "XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR" ;; esac

QB="--session --dest org.querencia.Benachrichtigungen --object-path /org/freedesktop/Notifications"
qcall() { local m=$1; shift; gdbus call $QB --method "org.querencia.Benachrichtigungen.$m" "$@" 2>&1; }
qprop() { gdbus call $QB --method org.freedesktop.DBus.Properties.Get org.querencia.Benachrichtigungen "$1" 2>&1 | sed -E 's/^\(<(uint32 )?(.*)>,\)$/\2/'; }
qset() { gdbus call $QB --method org.freedesktop.DBus.Properties.Set org.querencia.Benachrichtigungen "$1" "$2" >/dev/null 2>&1; }
hat_besitzer() { gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus --method org.freedesktop.DBus.NameHasOwner "$1" 2>&1 | grep -q true; }
anzahl() { qprop Anzahl; }
ungelesen() { qprop Ungelesen; }
offen() { qprop ZentraleOffen; }
sichtbar() { "$BIN" --nicht-stoeren status 2>&1 | sed -n 's/^Sichtbare Popups: //p'; }
# gdbus schreibt den Typ nur beim ersten Element: „[(uint32 6, int64 …), (4, …)“
liste_ids() { qcall Liste | grep -oE "\((uint32 )?[0-9]+, (int64 )?[0-9]+, '" | sed -E 's/^\((uint32 )?([0-9]+),.*/\2/'; }
schliessen() { gdbus call --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications --method org.freedesktop.Notifications.CloseNotification "$1" >/dev/null 2>&1; }
warte_bis() { # Sekunden Befehl…
    local n=$(($1 * 5)); shift
    while [ $n -gt 0 ]; do "$@" && return 0; sleep 0.2; n=$((n - 1)); done
    return 1
}
starte_dienst() {
    QB_DEBUG=1 "$BIN" >>"$LOG" 2>&1 &
    DIENST=$!
    warte_bis 5 hat_besitzer org.querencia.Benachrichtigungen
}
beende_dienst() { kill "$DIENST" 2>/dev/null; wait "$DIENST" 2>/dev/null; warte_bis 5 bash -c "! gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus --method org.freedesktop.DBus.NameHasOwner org.freedesktop.Notifications | grep -q true"; }
signale() { grep -c "member=$1" "$OUT/q-signale-$S.log"; }

dbus-monitor "type='signal',interface='org.freedesktop.Notifications'" >"$OUT/signale-$S.log" 2>&1 &
MON1=$!
dbus-monitor "type='signal',path='/org/freedesktop/Notifications',interface='org.querencia.Benachrichtigungen'" \
    "type='signal',path='/org/freedesktop/Notifications',interface='org.freedesktop.DBus.Properties'" >"$OUT/q-signale-$S.log" 2>&1 &
MON2=$!
"$QA" "$GROESSE" 2>>"$OUT/qausgabe.log"; sleep 1
"$ATTRAPPE" 32 >"$OUT/attrappe.log" 2>&1 &
ATTR=$!
sleep 1.5

# Z01 Dienst, beide Namen
: >"$LOG"
starte_dienst
pruefe "Z01 Dienst bereit, Namen org.freedesktop.Notifications und org.querencia.Benachrichtigungen" \
    eval 'hat_besitzer org.freedesktop.Notifications && hat_besitzer org.querencia.Benachrichtigungen && grep -q "bereit:" "$LOG"'
grep -q "Verlauf $JSON: 0 Einträge" "$LOG" && ok "Z01 leerer Verlauf beim ersten Start ($JSON)" || nok "Z01 Verlauf-Log: $(grep Verlauf "$LOG" | head -1)"

# Z02 Introspektion
gdbus introspect $QB >"$OUT/introspect-zentrale-$S.txt" 2>&1
fehlt=""
for n in Liste AlsGelesen Entfernen AlleEntfernen ZentraleZeigen ZentraleSchliessen NichtStoeren Ungelesen Anzahl ZentraleOffen \
    "Neu(" "Geaendert(" "Zaehler(" SetzeNichtStoeren Zustand Verlauf VerlaufLoeschen ZustandGeaendert VerlaufGeaendert; do
    grep -q "$n" "$OUT/introspect-zentrale-$S.txt" || fehlt="$fehlt $n"
done
[ -z "$fehlt" ] && ok "Z02 Introspektion: neue und alte Methoden, Eigenschaften, Signale" || nok "Z02 fehlt:$fehlt"

# Z03 Meldungen verschiedener Programme, eine transiente, eine kritische
notify-send -a Mail -i mail-unread "Neue Mail von Anna" "Betreff: Treffen am Donnerstag" ; sleep 0.2
notify-send -a Mail -i mail-unread "Neue Mail von Ben" "Betreff: <b>Fotos</b> vom Wochenende" ; sleep 0.2
notify-send -a Kalender -i x-office-calendar "Termin in 15 Minuten" "Teamrunde, Raum 2" ; sleep 0.2
python3 -I "$MELDEN" -n Software -i system-software-update -a default=Öffnen -a jetzt="Jetzt installieren" \
    "Aktualisierungen verfügbar" "12 Pakete können aktualisiert werden" >/dev/null 2>&1 ; sleep 0.2
notify-send -e -a Lautstärke "Lautstärke 40 %" ; sleep 0.2
IDK=$(notify-send -p -u critical -a Akku -i battery-caution "Akku fast leer" "Noch 5 % – bitte Netzteil anschließen")
sleep 0.4
zeiger 200 $((AUSH - 100)); q "wait 300"
shot z01-popups-hinweis
[ "$(sichtbar)" -ge 5 ] && ok "Z03 Popups sichtbar ($(sichtbar)), Hinweis „in der Zentrale“ (Bild z01)" || nok "Z03 sichtbar=$(sichtbar)"
[ "$(anzahl)" = 5 ] && [ "$(ungelesen)" = 5 ] && ok "Z03 Anzahl 5, Ungelesen 5 (sechs gesendet, die transiente nicht gespeichert)" || nok "Z03 Anzahl=$(anzahl) Ungelesen=$(ungelesen)"
qcall Liste >"$OUT/liste-1-$S.txt"
! grep -q "Lautstärke 40" "$OUT/liste-1-$S.txt" && grep -q "Akku fast leer" "$OUT/liste-1-$S.txt" && ok "Z03 Liste ohne transiente, neueste zuerst: $(head -c 120 "$OUT/liste-1-$S.txt")" || nok "Z03 Liste: $(head -c 300 "$OUT/liste-1-$S.txt")"
[ "$(liste_ids | head -1)" = "$IDK" ] && ok "Z03 neueste (ID $IDK) steht in Liste vorn" || nok "Z03 erste ID $(liste_ids | head -1) statt $IDK"

# Z04 Persistenz
if [ -f "$JSON" ] && [ "$(stat -c %a "$JSON")" = 600 ] && [ "$(grep -c '"id":' "$JSON")" = 5 ] && ! grep -q "Lautstärke 40" "$JSON"; then
    ok "Z04 $JSON: 0600, 5 Einträge, ohne transiente"
else nok "Z04 JSON: $(ls -l "$JSON" 2>&1) ids=$(grep -c '"id":' "$JSON" 2>/dev/null)"; fi
cp "$JSON" "$OUT/benachrichtigungen-1-$S.json"

# Z05 Signale
[ "$(signale Neu)" = 5 ] && [ "$(signale Zaehler)" -ge 5 ] && grep -q '"Ungelesen"' "$OUT/q-signale-$S.log" \
    && ok "Z05 Signale: Neu 5×, Zaehler $(signale Zaehler)×, PropertiesChanged Ungelesen" || nok "Z05 Neu=$(signale Neu) Zaehler=$(signale Zaehler)"

# Z06 normale laufen ab, kritische bleibt; dann per Aufruf schließen
sleep 5.5
[ "$(sichtbar)" = 1 ] && ok "Z06 nach 5 s nur noch die kritische Meldung (Standarddauer unverändert)" || nok "Z06 sichtbar=$(sichtbar)"
schliessen "$IDK"; sleep 0.5
[ "$(sichtbar)" = 0 ] && [ "$(anzahl)" = 5 ] && ok "Z06 Popup geschlossen, Verlauf behält alle 5" || nok "Z06 sichtbar=$(sichtbar) anzahl=$(anzahl)"

# Z07 --zentrale zeigt das Fenster des laufenden Dienstes
"$BIN" --zentrale >"$OUT/zentrale-aufruf-$S.txt" 2>&1; RC=$?
warte_bis 3 eval '[ "$(offen)" = true ]'
zeiger 5 $((AUSH - 5))
shot z02-zentrale 1.0
[ $RC = 0 ] && [ "$(offen)" = true ] && ok "Z07 --zentrale (Exit 0) öffnet das Fenster, ZentraleOffen=true (Bild z02)" || nok "Z07 Exit $RC offen=$(offen) $(cat "$OUT/zentrale-aufruf-$S.txt")"
grep -q "Zentrale: 5 von 5 Einträgen angezeigt" "$LOG" && ok "Z07 Liste zeigt 5 von 5" || nok "Z07 $(grep 'Zentrale:' "$LOG" | tail -1)"
grep -q '"ZentraleOffen"' "$OUT/q-signale-$S.log" && ok "Z07 PropertiesChanged ZentraleOffen" || nok "Z07 kein PropertiesChanged ZentraleOffen"

# Z08 Entf entfernt den gewählten (obersten) Eintrag
q "key Delete" "wait 600"
[ "$(anzahl)" = 4 ] && ! liste_ids | grep -qx "$IDK" && ok "Z08 Entf: oberster Eintrag (Akku, ID $IDK) entfernt, Anzahl 4" || nok "Z08 anzahl=$(anzahl) ids=$(liste_ids | tr '\n' ' ')"
[ "$(signale Geaendert)" -ge 1 ] && ok "Z08 Signal Geaendert" || nok "Z08 kein Geaendert"
shot z03-nach-entf

# Z09 Eingabe auf dem nun gewählten Eintrag (Software): Standardaktion + gelesen
IDS=$(liste_ids | head -1)
U0=$(ungelesen)
q "key Return" "wait 700"
if grep -A2 "member=ActionInvoked" "$OUT/signale-$S.log" | grep -q '"default"' && [ "$(ungelesen)" = $((U0 - 1)) ]; then
    ok "Z09 Eingabe: ActionInvoked($IDS, default), Eintrag gelesen (Ungelesen $(ungelesen))"
else nok "Z09 Eingabe: ungelesen=$(ungelesen) (vorher $U0), Signale: $(grep -c ActionInvoked "$OUT/signale-$S.log")"; fi

# Z10 Tab auf den Aktionsknopf „Jetzt installieren“, Leertaste
q "key Tab" "wait 300" "key space" "wait 700"
grep -A2 "member=ActionInvoked" "$OUT/signale-$S.log" | grep -q '"jetzt"' && ok "Z10 Aktionsknopf in der Zentrale: ActionInvoked($IDS, jetzt)" || nok "Z10 keine Aktion jetzt"
shot z04-nach-aktion

# Z11 Suche (Strg+F), Esc leert, zweites Esc schließt
q "chord ctrl f" "wait 300" "type Mail" "wait 900"
shot z05-suche
grep -q "Zentrale: 2 von 4 Einträgen angezeigt" "$LOG" && ok "Z11 Suche „Mail“: 2 von 4 (Bild z05)" || nok "Z11 Suche: $(grep 'Zentrale:' "$LOG" | tail -1)"
q "key Escape" "wait 700"
[ "$(offen)" = true ] && grep "Zentrale:" "$LOG" | tail -1 | grep -q "4 von 4" && ok "Z11 Esc im Suchfeld leert die Suche, Fenster bleibt" || nok "Z11 offen=$(offen) $(grep 'Zentrale:' "$LOG" | tail -1)"
q "key Escape" "wait 700"
[ "$(offen)" = false ] && ok "Z11 zweites Esc schließt das Fenster (ZentraleOffen=false)" || nok "Z11 offen=$(offen)"

# Z12 ZentraleZeigen per D-Bus, AlsGelesen per D-Bus, Esc schließt aus der Liste
qcall ZentraleZeigen "" >/dev/null
warte_bis 3 eval '[ "$(offen)" = true ]' && ok "Z12 ZentraleZeigen öffnet wieder" || nok "Z12 offen=$(offen)"
IDM=$(liste_ids | tail -1)
U0=$(ungelesen)
qcall AlsGelesen "@au [$IDM]" >/dev/null; sleep 0.5
[ "$(ungelesen)" = $((U0 - 1)) ] && ok "Z12 AlsGelesen([$IDM]): Ungelesen $U0 → $(ungelesen)" || nok "Z12 ungelesen=$(ungelesen)"
qcall AlsGelesen "@au [$IDM]" >/dev/null; sleep 0.3
[ "$(ungelesen)" = $((U0 - 1)) ] && ok "Z12 zweites AlsGelesen ändert nichts" || nok "Z12 ungelesen=$(ungelesen)"
shot z06-gelesen
q "key Escape" "wait 600"
[ "$(offen)" = false ] && ok "Z12 Esc in der Liste schließt" || nok "Z12 offen=$(offen)"

# Zugriffstasten je Sprache: „_Nicht stören“/„Do Not _Disturb“,
# „Alle _entfernen“/„C_lear All“.
case $SPRACHE in de) T_NS=n; T_ALLE=e ;; *) T_NS=d; T_ALLE=l ;; esac

# Z13 Nicht stören: per Eigenschaft und per Alt+N/Alt+D im Fenster; kritisch kommt durch
qset NichtStoeren "<true>"; sleep 0.4
[ "$(qprop NichtStoeren)" = true ] && "$BIN" --nicht-stoeren status | grep -q "Nicht stören: an" && ok "Z13 Eigenschaft NichtStoeren=true, CLI zeigt an" || nok "Z13 $(qprop NichtStoeren)"
grep -q '"NichtStoeren"' "$OUT/q-signale-$S.log" && ok "Z13 PropertiesChanged NichtStoeren" || nok "Z13 kein PropertiesChanged NichtStoeren"
A0=$(anzahl)
notify-send -a Chat "Nachricht von Clara" "Kommst du heute?"; sleep 0.8
[ "$(sichtbar)" = 0 ] && [ "$(anzahl)" = $((A0 + 1)) ] && ok "Z13 Nicht stören: normale Meldung ohne Popup, im Verlauf" || nok "Z13 sichtbar=$(sichtbar) anzahl=$(anzahl)"
IDK2=$(notify-send -p -u critical -a Akku "Akku kritisch" "Noch 2 %"); sleep 0.8
[ "$(sichtbar)" = 1 ] && ok "Z13 Nicht stören: kritische Meldung erscheint trotzdem" || nok "Z13 kritisch sichtbar=$(sichtbar)"
"$BIN" --zentrale >/dev/null 2>&1; sleep 1
shot z07-nicht-stoeren
schliessen "$IDK2"
q "chord alt $T_NS" "wait 600"
[ "$(qprop NichtStoeren)" = false ] && ok "Z13 Alt+$T_NS im Fenster schaltet Nicht stören aus" || nok "Z13 Alt+$T_NS: $(qprop NichtStoeren)"
q "chord alt $T_NS" "wait 600"
[ "$(qprop NichtStoeren)" = true ] && ok "Z13 Alt+$T_NS schaltet wieder an" || nok "Z13 Alt+$T_NS 2: $(qprop NichtStoeren)"
gdbus call $QB --method org.querencia.Benachrichtigungen.SetzeNichtStoeren false >/dev/null 2>&1; sleep 0.4
[ "$(qprop NichtStoeren)" = false ] && ok "Z13 alte Methode SetzeNichtStoeren(false) wirkt" || nok "Z13 SetzeNichtStoeren: $(qprop NichtStoeren)"
q "key Escape" "wait 500"

# Z14 ini: anzeigen=nein (kritisch kommt durch), Positionen, dauer_s, hinweis
mkdir -p "$(dirname "$INI")"
printf '[Einblendung]\nanzeigen=nein\n' >"$INI"
A0=$(anzahl)
notify-send -a Probe "Ohne Einblendung" "nur im Verlauf"; sleep 0.8
[ "$(sichtbar)" = 0 ] && [ "$(anzahl)" = $((A0 + 1)) ] && grep -q "Einstellungen aus $INI: anzeigen=nein" "$LOG" && ok "Z14 anzeigen=nein: kein Popup, im Verlauf, ini neu gelesen" || nok "Z14 sichtbar=$(sichtbar) anzahl=$(anzahl)"
IDK3=$(notify-send -p -u critical -a Probe "Kritisch trotz anzeigen=nein"); sleep 0.8
[ "$(sichtbar)" = 1 ] && ok "Z14 anzeigen=nein: kritische erscheint" || nok "Z14 kritisch sichtbar=$(sichtbar)"
schliessen "$IDK3"; sleep 0.4
printf '[Einblendung]\nposition=unten-rechts\n' >"$INI"; touch -d '+2 seconds' "$INI"
notify-send -a Probe -t 0 "Unten rechts" "Position aus der ini"; sleep 0.9
shot z08-unten-rechts
UNTEN=$(python3 -I "$BILD" "$OUT/z08-unten-rechts-$S.png" popup-pixel $((AUSW - 380)) $((AUSH - 200)) "$AUSW" "$AUSH")
OBEN=$(python3 -I "$BILD" "$OUT/z08-unten-rechts-$S.png" popup-pixel $((AUSW - 380)) 0 "$AUSW" 200)
[ "$UNTEN" -gt 5000 ] && [ "$OBEN" -lt 500 ] && ok "Z14 position=unten-rechts (Popup-Pixel unten $UNTEN, oben $OBEN; Bild z08)" || nok "Z14 unten=$UNTEN oben=$OBEN"
gdbus call --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications --method org.freedesktop.Notifications.CloseNotification "$(liste_ids | head -1)" >/dev/null 2>&1; sleep 0.5
printf '[Einblendung]\nposition=oben-mitte\nhinweis=nein\n' >"$INI"; touch -d '+4 seconds' "$INI"
IDM2=$(notify-send -p -a Probe -t 0 "Oben in der Mitte" "ohne Hinweis"); sleep 0.9
shot z09-oben-mitte
MITTE=$(python3 -I "$BILD" "$OUT/z09-oben-mitte-$S.png" popup-pixel $((AUSW / 2 - 100)) 30 $((AUSW / 2 + 100)) 160)
RECHTS=$(python3 -I "$BILD" "$OUT/z09-oben-mitte-$S.png" popup-pixel $((AUSW - 60)) 30 "$AUSW" 160)
[ "$MITTE" -gt 5000 ] && [ "$RECHTS" -lt 200 ] && ok "Z14 position=oben-mitte (Mitte $MITTE, rechter Rand $RECHTS; Bild z09 ohne Hinweis)" || nok "Z14 mitte=$MITTE rechts=$RECHTS"
schliessen "$IDM2"; sleep 0.5
printf '[Einblendung]\ndauer_s=2\n' >"$INI"; touch -d '+6 seconds' "$INI"
notify-send -a Probe "Zwei Sekunden" "dauer_s=2"; sleep 1.2
S1=$(sichtbar); sleep 1.6
[ "$S1" = 1 ] && [ "$(sichtbar)" = 0 ] && ok "Z14 dauer_s=2: nach 1,2 s sichtbar, nach 2,8 s weg" || nok "Z14 dauer: $S1 → $(sichtbar)"

# Z15 verlauf=20 kürzt und begrenzt
printf '[Zentrale]\nverlauf=20\n' >"$INI"; touch -d '+8 seconds' "$INI"
for i in $(seq 1 25); do notify-send -e -a Last "transient $i"; done
notify-send -a Last -u low "Kürzen auslösen"; sleep 0.3
for i in $(seq 1 24); do notify-send -a Last -u low "Last $i"; done
sleep 1
[ "$(anzahl)" = 20 ] && ok "Z15 verlauf=20: Anzahl 20 (nach 25 weiteren)" || nok "Z15 anzahl=$(anzahl)"
"$BIN" --verlauf >"$OUT/verlauf-cli-$S.txt" 2>&1
[ "$(tail -1 "$OUT/verlauf-cli-$S.txt")" = "20 Einträge" ] && ok "Z15 --verlauf (alte Methode): 20 Einträge" || nok "Z15 --verlauf: $(tail -1 "$OUT/verlauf-cli-$S.txt")"
rm -f "$INI"; sleep 3

# Z16 Alle entfernen im Fenster: Zugriffstaste, Rückfrage, Eingabe
"$BIN" --zentrale >/dev/null 2>&1; sleep 1
q "chord alt $T_ALLE" "wait 900"
shot z10-rueckfrage
q "key Return" "wait 900"
[ "$(anzahl)" = 0 ] && [ "$(ungelesen)" = 0 ] && ok "Z16 Alt+$T_ALLE, Rückfrage, Eingabe: Verlauf leer (Bild z10)" || nok "Z16 anzahl=$(anzahl)"
shot z11-leer
grep -q '"id":' "$JSON" && nok "Z16 JSON noch mit Einträgen" || ok "Z16 JSON leer"
q "key Escape" "wait 500"

# Z17 Neustart: Einträge, gelesen und IDs bleiben, Aktionen nicht mehr anbietbar
python3 -I "$MELDEN" -n Software -a default=Öffnen -a jetzt="Jetzt installieren" "Nach Neustart" "Aktionen nicht mehr anbieten" >/dev/null
notify-send -a Mail "Alte Mail 1" "gelesen"; notify-send -a Mail "Alte Mail 2" "ungelesen"
sleep 0.5
qcall AlsGelesen "@au [$(liste_ids | sed -n 2p)]" >/dev/null
MAXID=$(liste_ids | sort -n | tail -1)
U0=$(ungelesen); A0=$(anzahl)
beende_dienst; sleep 0.5
starte_dienst
[ "$(anzahl)" = "$A0" ] && [ "$(ungelesen)" = "$U0" ] && ok "Z17 nach Neustart: Anzahl $A0, Ungelesen $U0 aus der Datei" || nok "Z17 anzahl=$(anzahl)/$A0 ungelesen=$(ungelesen)/$U0"
NEU=$(notify-send -p -a Probe "Nach dem Neustart")
[ "$NEU" -gt "$MAXID" ] && ok "Z17 neue ID $NEU > höchste alte $MAXID" || nok "Z17 neue ID $NEU, alte $MAXID"
sleep 5.5
"$BIN" --zentrale >/dev/null 2>&1; sleep 1.2
shot z12-nach-neustart

# Z18 --zentrale ohne Dienst: der Aufruf wird der Dienst
beende_dienst; sleep 0.5
hat_besitzer org.freedesktop.Notifications && nok "Z18 Name noch belegt" || ok "Z18 Dienst beendet, Name frei"
QB_DEBUG=1 "$BIN" --zentrale >>"$LOG" 2>&1 &
DIENST=$!
warte_bis 5 eval '[ "$(offen)" = true ]'
hat_besitzer org.freedesktop.Notifications && [ "$(offen)" = true ] && ok "Z18 --zentrale ohne Dienst: startet den Dienst und zeigt die Zentrale" || nok "Z18 offen=$(offen)"
shot z13-zentrale-startet-dienst
beende_dienst

# Z19 --zentrale bei einem älteren Dienst (0.1.3 aus QB_ALT): Hinweisfenster, Exit 1
if [ -n "${QB_ALT:-}" ] && [ -x "$QB_ALT" ]; then
    "$QB_ALT" >"$OUT/dienst-alt-$S.log" 2>&1 &
    ALT=$!
    warte_bis 5 hat_besitzer org.freedesktop.Notifications
    "$BIN" --zentrale >"$OUT/zentrale-alt-$S.txt" 2>&1 &
    Z=$!
    sleep 1.5
    shot z14-aelterer-dienst
    q "key Return" "wait 700"
    wait "$Z"; RC=$?
    [ $RC = 1 ] && grep -q "ohne Mitteilungszentrale" "$OUT/zentrale-alt-$S.txt" && ok "Z19 älterer Dienst: Hinweisfenster (Bild z14), Exit 1" || nok "Z19 Exit $RC $(cat "$OUT/zentrale-alt-$S.txt")"
    kill "$ALT" 2>/dev/null; wait "$ALT" 2>/dev/null
else
    info "Z19 übersprungen (QB_ALT nicht gesetzt)"
fi

grep -i -E 'critical|warning' "$LOG" | grep -v "Gtk-WARNING.*Theme parser" | head -20 >"$OUT/gtk-meldungen-zentrale-$S.txt"
info "GTK-Meldungen im Dienst-Log: $(wc -l <"$OUT/gtk-meldungen-zentrale-$S.txt")"
kill "$ATTR" "$MON1" "$MON2" 2>/dev/null
echo "Ergebnis $GROESSE $SPRACHE: $(grep -c '^PASS' "$R") PASS, $(grep -c '^FAIL' "$R") FAIL" | tee -a "$R"
touch "$HMP04_PROFILE/logs/FERTIG"
