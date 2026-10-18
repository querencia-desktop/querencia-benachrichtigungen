# Querencia Benachrichtigungen

Dienst für Desktop-Benachrichtigungen nach der Freedesktop-Spezifikation
(`org.freedesktop.Notifications`, Version 1.2) für Querencia Desktop
(labwc 0.9.8, Wayland). Rust, GTK 4, gtk4-layer-shell, zbus.

- Popups oben rechts unter dem oberen Panel (Layer-Shell, Ebene overlay,
  Exklusivzone 0: der Compositor rückt die Popups aus der Panel-Exklusivzone
  heraus, die Panelhöhe wird nicht geraten), Stapel nach unten, höchstens
  5 sichtbar, weitere warten.
- Dringlichkeit: niedrig 3 s, normal 5 s, kritisch mit orangem Rand ohne
  Ablauf. Zeiger über der Meldung hält die Zeit an. Klick löst die
  Default-Aktion aus, Aktionsknöpfe senden `ActionInvoked`.
- Hints: `urgency`, `image-path`, `image-data`, `desktop-entry`,
  `transient`, `resident`, `category`. Body-Markup (`b`, `i`, `u`, `a`)
  wird sicher gerendert, alles andere als Text gezeigt.
- Eigene Schnittstelle `org.querencia.Benachrichtigungen` am selben Pfad:
  `SetzeNichtStoeren(b)`, `Zustand() -> (b, u, u)`, `Verlauf() ->
  a(ussssyx)`, `VerlaufLoeschen()`, Signale `ZustandGeaendert`,
  `VerlaufGeaendert` – für ein späteres Panel-Applet.

## Aufruf

    querencia-benachrichtigungen                      Dienst
    querencia-benachrichtigungen --nicht-stoeren an|aus|status
    querencia-benachrichtigungen --verlauf

Ist `org.freedesktop.Notifications` schon belegt, meldet der Dienst den
Besitzer und beendet sich mit Exit 1.

## Bauen und prüfen

    cargo build --release -j2
    cargo test
    bash werkzeug/gui-pruefung.sh              # isoliert, eigener labwc + D-Bus

Siehe `integration/` für den vorbereiteten Autostart-Patch und das
Integrationsskript (nur nach Freigabe des Besitzers ausführen).
