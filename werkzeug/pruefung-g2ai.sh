#!/bin/bash
# G2-AI (QD-062): Benachrichtigungen ohne erreichbaren Compositor, nur mit
# X11 und nach verlorener Wayland-Verbindung. Eigener D-Bus
# (dbus-run-session), eigener headless labwc, eigenes XDG_RUNTIME_DIR;
# der Dienst der laufenden Sitzung bleibt unberührt.
#   bash werkzeug/pruefung-g2ai.sh <binary> <ausgabeordner>
set -u
if [ "${1-}" != --innen ]; then
    BIN=$(realpath "$1"); OUT=$(realpath -m "$2"); mkdir -p "$OUT"
    P=$OUT/profil; rm -rf "$P"; mkdir -p "$P/data/dbus-1/services" "$P/config" "$P/state"
    for n in org.freedesktop.portal.Desktop org.freedesktop.portal.Documents; do
        printf '[D-BUS Service]\nName=%s\nExec=/bin/false\n' "$n" >"$P/data/dbus-1/services/$n.service"
    done
    RT=$(mktemp -d /tmp/qed-runtime-XXXXXX)
    env -i HOME="$HOME" PATH=/usr/bin:/bin LANG=en_US.UTF-8 LANGUAGE=de XDG_RUNTIME_DIR="$RT" \
        XDG_DATA_HOME="$P/data" XDG_CONFIG_HOME="$P/config" XDG_STATE_HOME="$P/state" XDG_DATA_DIRS=/usr/share \
        GDK_DEBUG=no-portals BIN="$BIN" OUT="$OUT" \
        timeout 90 dbus-run-session -- bash "$(realpath "$0")" --innen >"$OUT/lauf.log" 2>&1
    for p in /proc/[0-9]*; do
        tr '\0' '\n' 2>/dev/null <"$p/environ" | grep -qx "XDG_RUNTIME_DIR=$RT" && kill -TERM "${p#/proc/}" 2>/dev/null
    done
    rm -rf "$RT"
    cat "$OUT/ergebnis.txt"
    echo "Ergebnis: $(grep -c '^PASS' "$OUT/ergebnis.txt") PASS, $(grep -c '^FAIL' "$OUT/ergebnis.txt") FAIL"
    exit 0
fi
R=$OUT/ergebnis.txt; : >"$R"
ok() { echo "PASS $*" >>"$R"; }
nok() { echo "FAIL $*" >>"$R"; }
NAME=org.freedesktop.Notifications
besitzer() { busctl --user call org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus GetConnectionUnixProcessID s "$NAME" 2>/dev/null | awk '{print $2}'; }
# wartet bis $1 s auf das Ende von PID $2; Exit-Code in RC, Dauer in D
warte_ende() {
    local t0; t0=$(date +%s%N); RC=läuft
    for _ in $(seq 1 $(( $1 * 20 ))); do
        if ! kill -0 "$2" 2>/dev/null; then wait "$2"; RC=$?; break; fi
        sleep 0.05
    done
    D=$(( ($(date +%s%N) - t0) / 1000000 ))
}

# N01 kein Compositor (kein WAYLAND_DISPLAY, kein DISPLAY)
env -u WAYLAND_DISPLAY -u DISPLAY "$BIN" >"$OUT/n01.log" 2>&1 & p=$!
warte_ende 10 $p
[ "$RC" != läuft ] && [ "$RC" != 0 ] && [ -z "$(besitzer)" ] &&
    ok "N01 ohne Compositor: beendet nach $D ms (Exit $RC), $NAME frei: $(grep -o 'kein Wayland[^;]*' "$OUT/n01.log" | head -1)" ||
    nok "N01 Exit $RC, Besitzer '$(besitzer)': $(tr '\n' ';' <"$OUT/n01.log")"

# headless labwc mit Xwayland für N02–N04
mkdir -p "$OUT/labwc"; echo '<labwc_config/>' >"$OUT/labwc/rc.xml"
WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 \
    ~/opt/labwc-0.9.8/bin/labwc -C "$OUT/labwc" -s "sh -c 'echo \$WAYLAND_DISPLAY \$DISPLAY >$OUT/sockets.txt; sleep 600'" \
    >"$OUT/labwc.log" 2>&1 & L=$!
for _ in $(seq 1 50); do [ -s "$OUT/sockets.txt" ] && break; sleep 0.1; done
read -r WD XD <"$OUT/sockets.txt"

# N02 nur X11 (Xwayland des isolierten labwc) angeboten, der Wayland-Socket
# fehlt (ohne WAYLAND_DISPLAY nähme GDK wayland-0): kein Rückfall auf X11
if [ -n "${XD:-}" ]; then
    WAYLAND_DISPLAY=wayland-fehlt DISPLAY="$XD" "$BIN" >"$OUT/n02.log" 2>&1 & p=$!
    warte_ende 15 $p
    [ "$RC" != läuft ] && [ "$RC" != 0 ] && [ -z "$(besitzer)" ] && ! grep -q 'bereit' "$OUT/n02.log" &&
        ok "N02 nur X11 ($XD): kein Rückfall, beendet nach $D ms (Exit $RC), $NAME frei" ||
        nok "N02 Exit $RC, Besitzer '$(besitzer)': $(tr '\n' ';' <"$OUT/n02.log")"
else
    nok "N02 kein Xwayland im isolierten labwc ($(cat "$OUT/sockets.txt"))"
fi

# N03 Wayland da: Dienst belegt den Namen
WAYLAND_DISPLAY=$WD "$BIN" >"$OUT/n03.log" 2>&1 & d=$!
for _ in $(seq 1 100); do grep -q bereit "$OUT/n03.log" && break; sleep 0.1; done
[ "$(besitzer)" = "$d" ] && ok "N03 mit Wayland ($WD): bereit, $NAME gehört PID $d" || nok "N03 Besitzer '$(besitzer)' statt $d: $(tr '\n' ';' <"$OUT/n03.log")"

# N04 Verbindung verloren: labwc endet, Dienst endet sofort, Name frei
kill -KILL $L
warte_ende 5 $d
[ "$RC" != läuft ] && [ -z "$(besitzer)" ] &&
    ok "N04 labwc beendet: Dienst endet nach $D ms (Exit $RC), $NAME frei ($(grep -o -E 'Wayland-Verbindung [a-z]+|Lost connection[^.]*' "$OUT/n03.log" | head -1))" ||
    nok "N04 Exit $RC nach $D ms, Besitzer '$(besitzer)': $(tail -3 "$OUT/n03.log" | tr '\n' ';')"
