# Änderungen

## 0.2.2 – 2026-10-21

Kontrast hell (QD-089, Kontrast-Sammelauftrag):

- Programmname und Hinweis „in der Zentrale“ im Popup hell `#6f6a64`
  statt `#7a756f` auf `#f9f8f6`: 5,0:1 statt 4,3:1 (WCAG AA 4,5:1).
  Gleicher Farbton, nur dunkler; Überfahren bleibt `#3c3c3c`.
- Dunkel unverändert (`#b3aea7` auf `#34332f`, 5,7:1). Die
  Mitteilungszentrale hat keine eigene Nebenschrift-Farbe (Zeit und Anzahl
  mit 75 % der Themenschrift; im Bild gemessen hell 10,4:1 in der Liste und
  5,1:1 auf der Auswahl) und bleibt unverändert.
- Neuer Unit-Test `programmname_und_hinweis_erreichen_aa` (hell und dunkel).
- Sonst pixelgleich mit 0.2.1; Texte und Katalog unverändert.

## 0.2.1 – 2026-10-21

Dunkle Darstellung (CSS-Welle dunkel, QD-081, mit Querencia-Klassisch 0.2):

- Popups haben eine dunkle Fassung für „Dunkle Darstellung bevorzugen“
  (`color-scheme prefer-dark`): Grund `#34332f`, Rand `#23221f`, Schrift
  `#f2f1f0`/`#dfdbd2`, Programmname und Hinweis `#b3aea7` (Kontrast AA).
  Der kritische Rand bleibt orange. Der eigene CSS-Anbieter ist dafür an
  `gtk-interface-color-scheme` gebunden; ohne Bindung blieb der Dunkelblock
  in jeder Lage wirkungslos (isoliert gemessen).
- Das Farbschema gilt auch nach dem Start: GTK 4.22 wendet ein beim Start
  schon dunkles Schema erst nach einem Wechsel an; der Dienst meldet
  `gtk-interface-color-scheme` einmal nach `gtk4::init()` (auch im
  Hinweisfenster von `--zentrale`).
- Die Mitteilungszentrale hat keine eigenen Farben außer dem orangen Rand
  kritischer Einträge und folgt dem Thema.
- Hell unverändert (pixelgleich mit 0.2.0); Texte und Katalog unverändert.

## 0.2.0 – 2026-10-20

Mitteilungszentrale (Wunsch: Mitteilungen nicht nur schwebend einblenden):

- Verlauf aller Meldungen außer `transient` als Ringpuffer (Standard 200)
  mit gelesen/ungelesen, gespeichert in
  `~/.local/state/querencia/benachrichtigungen.json` (0600, atomar
  geschrieben; eine unlesbare Datei wird als `.defekt` beiseitegelegt).
  Nach einem Neustart vergibt der Dienst nur IDs oberhalb der
  gespeicherten. Bilder aus `image-data` bleiben verkleinert im Speicher.
- Fenster „Mitteilungszentrale“ im Dienstprozess: schlanke Menüleiste,
  Suchfeld, „Nicht stören“, „Alle entfernen“ (mit Rückfrage), Liste nach
  Programm gruppiert und neueste oben, Einträge mit Symbol, Titel, Text, Zeit,
  Aktionen, „Als gelesen“, „Entfernen“, Statuszeile. Tasten Esc, Entf,
  Eingabe, Strg+F. Farben aus dem Thema der Sitzung.
- Aufruf `--zentrale`; Starter `org.querencia.Benachrichtigungen.desktop`
  („Mitteilungszentrale“, Zubehör, Symbol `preferences-desktop-notification`).
  Programmname und app_id sind jetzt `org.querencia.Benachrichtigungen`.
- Zweiter Busname `org.querencia.Benachrichtigungen` mit `Liste`,
  `AlsGelesen`, `Entfernen`, `AlleEntfernen`, `ZentraleZeigen`,
  `ZentraleSchliessen`, Eigenschaften `NichtStoeren`, `Ungelesen`,
  `Anzahl`, `ZentraleOffen` und Signalen `Neu`, `Geaendert`, `Zaehler`
  (`docs/dbus.md`). Die Methoden aus 0.1.x bleiben; `Verlauf` liefert wie
  bisher die letzten 50.
- „Nicht stören“ lässt kritische Meldungen jetzt als Popup durch, ebenso
  `[Einblendung] anzeigen=nein` (0.1.x unterdrückte bei „Nicht stören“
  alle). Grund: Freedesktop-Konvention, Akku- und Sicherheitswarnungen
  sollen nicht ungesehen im Verlauf landen.
- ini `~/.config/querencia/benachrichtigungen.ini`: `[Einblendung]
  anzeigen, dauer_s, position (oben-rechts|oben-mitte|unten-rechts),
  hinweis`, `[Zentrale] verlauf`. Ohne Datei verhalten sich die Popups wie
  in 0.1.3, nur mit dem Hinweis „in der Zentrale“.
- Popups: Schließen-X, Klick und Aktion markieren die Meldung als gelesen.

## 0.1.3 – 2026-10-20

Kein Dienst ohne Wayland (QD-062, G2-AI):

- Nur das Wayland-Backend von GDK ist zugelassen. Ohne erreichbaren
  Compositor beendet sich der Dienst sofort, ohne
  `org.freedesktop.Notifications` zu belegen. Ein Rückfall auf X11
  (Xwayland) mit der CRITICAL-Meldung von gtk4-layer-shell kommt nicht
  mehr vor.
- Ist das Display keine Wayland-Verbindung oder fehlt wlr-layer-shell,
  beendet sich der Dienst ebenfalls vor dem Belegen des Namens.
- Wird die Wayland-Verbindung geschlossen, endet der Dienst sofort und
  gibt den Namen frei. Das gilt auch dann, wenn GDK den Prozess nicht
  selbst beendet.

## 0.1.2 – 2026-10-20

Englisch und Deutsch über GNU gettext (G2-AH, Muster Querencia Editor 0.2.0):

- Der einzige eigene sichtbare Text, der Tooltip am Schließen-Knopf der
  Popups, ist englisch im Code („Close“), Deutsch aus `po/de.po` mit dem
  bisherigen Wortlaut („Schließen“). Dienst, Popups, Schnittstellen und
  Befehlszeile sonst unverändert.
- `werkzeug/i18n.py` für die po-Pflege; GUI-Prüfung mit `QB_LANGUAGE`,
  `QB_GROESSE` und neuem Szenario `szenario-sprache.sh`; Quelltexte von
  `qeingabe`/`qausgabe` liegen jetzt im Repo (bauen byte-gleich nach).
- Doku unter `docs/` (`docs/building.md`, `docs/testing.md`), Lizenzkopf,
  dieses Änderungsprotokoll.

## 0.1.1 – 2026-10-18

Bilder aus `image-data`, `image-path` und `app_icon` werden auf 48 px
verkleinert (Electron/libnotify schickt 512×512), Betreff höchstens 2,
Text höchstens 5 Zeilen (3 mit Aktionsknöpfen), Breite bleibt 360 px.
Prüffälle für übergroße Eingaben (G2-L).

## 0.1.0 – 2026-10-18

Erster Stand (G2-L): Dienst `org.freedesktop.Notifications` 1.2 mit Popups
oben rechts (gtk4-layer-shell), Dringlichkeiten, Aktionen, Body-Markup,
Hints, „Nicht stören“ mit Verlauf über `org.querencia.Benachrichtigungen`,
Befehlszeile `--nicht-stoeren`, `--verlauf`. Isoliert geprüft (42 Fälle).
