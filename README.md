# Querencia Benachrichtigungen

Dienst für Desktop-Benachrichtigungen nach der Freedesktop-Spezifikation
(`org.freedesktop.Notifications`, Version 1.2) für Querencia Desktop
(labwc 0.9.8, Wayland), mit Mitteilungszentrale. Rust, GTK 4,
gtk4-layer-shell, zbus.

- Popups oben rechts unter dem oberen Panel (Layer-Shell, Ebene overlay,
  Exklusivzone 0: der Compositor rückt die Popups aus der Panel-Exklusivzone
  heraus, die Panelhöhe wird nicht geraten), Stapel nach unten, höchstens
  5 sichtbar, weitere warten. Position, Dauer und Ein/Aus stehen in der ini
  (siehe unten).
- Dringlichkeit: niedrig 3 s, normal 5 s, kritisch mit orangem Rand ohne
  Ablauf. Zeiger über der Meldung hält die Zeit an. Klick löst die
  Default-Aktion aus, Aktionsknöpfe senden `ActionInvoked`. Unten rechts im
  Popup steht der Hinweis „in der Zentrale“; ein Klick darauf öffnet sie.
- **Mitteilungszentrale (ab 0.2.0):** Jede Meldung außer `transient` kommt
  in einen Verlauf (Ringpuffer, Standard 200 Einträge, gespeichert in
  `~/.local/state/querencia/benachrichtigungen.json` mit Rechten 0600),
  gelesen oder ungelesen. Das Fenster „Mitteilungszentrale“ zeigt den
  Verlauf nach Programm gruppiert, neueste oben, mit Suchfeld, Aktionen der
  Meldung, „Als gelesen“, „Entfernen“, „Alle entfernen“ und dem Umschalter
  „Nicht stören“. Tasten: Esc schließt, Entf entfernt, Eingabe löst die
  Standardaktion aus, Strg+F sucht.
- „Nicht stören“ unterdrückt Popups außer kritischen; der Verlauf füllt
  sich weiter.
- Hints: `urgency`, `image-path`, `image-data`, `desktop-entry`,
  `transient`, `resident`, `category`. Body-Markup (`b`, `i`, `u`, `a`)
  wird sicher gerendert, alles andere als Text gezeigt.
- Eigene Schnittstelle `org.querencia.Benachrichtigungen` (eigener
  Busname, derselbe Pfad) für das Panel-Applet: `Liste`, `AlsGelesen`,
  `Entfernen`, `AlleEntfernen`, `ZentraleZeigen`, Eigenschaften
  `NichtStoeren`, `Ungelesen`, `Anzahl`, Signale `Neu`, `Geaendert`,
  `Zaehler`; die Methoden aus 0.1.x bleiben. Beschreibung:
  [docs/dbus.md](docs/dbus.md).

## Aufruf

    querencia-benachrichtigungen                      Dienst
    querencia-benachrichtigungen --zentrale           Mitteilungszentrale zeigen
    querencia-benachrichtigungen --nicht-stoeren an|aus|status
    querencia-benachrichtigungen --verlauf

Ist `org.freedesktop.Notifications` schon belegt, meldet der Dienst den
Besitzer und beendet sich mit Exit 1. `--zentrale` bittet den laufenden
Dienst, das Fenster zu zeigen. Läuft keiner, wird der Aufruf selbst zum
Dienst. Läuft ein älterer oder fremder Dienst, erklärt ein kleines Fenster,
warum es keine Zentrale gibt. Der Starter
`org.querencia.Benachrichtigungen.desktop` („Mitteilungszentrale“ unter
Zubehör) ruft `--zentrale` auf.

## Einstellungen

`~/.config/querencia/benachrichtigungen.ini` (bzw. unter
`$XDG_CONFIG_HOME`), optional; ohne Datei gelten die Standardwerte:

    [Einblendung]
    anzeigen=ja          # nein: nur noch kritische Meldungen als Popup
    dauer_s=5            # Standarddauer normaler Meldungen (niedrige: 3/5 davon), 0 = bis geschlossen
    position=oben-rechts # oben-rechts | oben-mitte | unten-rechts
    hinweis=ja           # Hinweis „in der Zentrale“ im Popup

    [Zentrale]
    verlauf=200          # Einträge im Verlauf (10 bis 1000)

Der Dienst liest die Datei neu, sobald sie sich ändert (geprüft bei jeder
Meldung und beim Öffnen der Zentrale). Eine Seite in den Einstellungen
folgt später.

Gestartet wird der Dienst vom labwc-Autostart der Querencia-Sitzung
(`querencia-runtime`, `session/querencia/labwc/autostart`) über den Link
`~/opt/querencia/bin/querencia-benachrichtigungen`.

Bauen: [docs/building.md](docs/building.md). Prüfen (Unit-Tests, isolierte
GUI mit eigenem labwc und eigenem D-Bus): [docs/testing.md](docs/testing.md).

## Sprachen

Seit 0.1.2 sind die Texte, die der Dienst selbst zeigt, englisch im Code
(msgid) und deutsch in `po/de.po` (GNU gettext, Muster Querencia Editor
0.2.0). Das sind die Mitteilungszentrale, der Starter, der Hinweis und der
Tooltip „Close“/„Schließen“ im Popup. Betreff, Text und Aktionen kommen vom
Absender. Die Sprache folgt `LANGUAGE`/`LANG` (die
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

