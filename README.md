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

Gestartet wird der Dienst vom labwc-Autostart der Querencia-Sitzung
(`querencia-runtime`, `session/querencia/labwc/autostart`) über den Link
`~/opt/querencia/bin/querencia-benachrichtigungen`.

Bauen: [docs/building.md](docs/building.md). Prüfen (Unit-Tests, isolierte
GUI mit eigenem labwc und eigenem D-Bus): [docs/testing.md](docs/testing.md).

## Sprachen

Seit 0.1.2 sind die Texte, die der Dienst selbst zeigt, englisch im Code
(msgid) und deutsch in `po/de.po` (GNU gettext, Muster Querencia Editor
0.2.0); der deutsche Wortlaut ist der bisherige. Das ist nur der Tooltip
„Close“/„Schließen“ am Schließen-Knopf der Popups – Betreff, Text und
Aktionen kommen vom Absender. Die Sprache folgt `LANGUAGE`/`LANG` (die
Sitzung setzt `LANGUAGE=de`); ohne deutschen Katalog erscheint Englisch.
Befehlszeile, `--hilfe`, Log, D-Bus-Fehlertexte und `GetServerInformation`
(„Querencia Benachrichtigungen“) bleiben unverändert.

## Lizenz

GNU GPL, Version 3 oder neuer, siehe [LICENSE](LICENSE). Copyright 2026
Querencia Desktop contributors. Eigener Code; aus notification-daemon wurde
kein Code übernommen. Änderungen: [CHANGELOG.md](CHANGELOG.md).

Eigene Vermerke: `werkzeug/virtual-keyboard-unstable-v1.xml` (gekürzte Kopie
des Wayland-Protokolls, Copyright Kristian Høgsberg u. a., MIT-artige Lizenz
im Dateikopf) und die mit `wayland-scanner` erzeugten Dateien unter
`werkzeug/bin/` (Lizenz der jeweiligen Protokolle); nur für das Prüfwerkzeug.

