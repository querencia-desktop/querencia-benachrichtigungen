# Querencia Benachrichtigungen

Dienst für Desktop-Benachrichtigungen für alle, die Querencia Desktop
benutzen: zeigt Meldungen anderer Programme als Popups unter dem oberen Panel
und sammelt sie in der Mitteilungszentrale. Teil von
[Querencia Desktop](https://querencialinux.org) – einer
klassischen Desktopumgebung im GNOME-2-Stil für Wayland (labwc, Rust, GTK 4).
Installiert wird es zusammen mit Querencia Desktop aus der APT-Quelle;
Anleitung auf <https://querencialinux.org>.

*English summary:* A desktop notification daemon (org.freedesktop.Notifications
1.2) with layer-shell popups, do-not-disturb and a notification centre with
history. Part of Querencia Desktop, a classic GNOME-2-style desktop for
Wayland. Alpha software; documentation is in German.

> **Alpha.** Eine Seite in den Einstellungen fehlt noch (Einstellungen über
> die ini-Datei). Keine Sicherheits- oder Datenverlustgarantie.

## Zweck

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
- **Dunkle Darstellung (ab 0.2.1):** Unter „Dunkle Darstellung bevorzugen“
  (`color-scheme prefer-dark`) sind Popups und Zentrale dunkel, beim Start
  und bei einem Wechsel im Lauf. Die Zentrale folgt dem GTK-Thema
  (Querencia-Klassisch ab 0.2), die Popups haben eigene helle und dunkle
  Farben.
- Hints: `urgency`, `image-path`, `image-data`, `desktop-entry`,
  `transient`, `resident`, `category`. Body-Markup (`b`, `i`, `u`, `a`)
  wird sicher gerendert, alles andere als Text gezeigt.
- Eigene Schnittstelle `org.querencia.Benachrichtigungen` (eigener
  Busname, derselbe Pfad) für das Panel-Applet: `Liste`, `AlsGelesen`,
  `Entfernen`, `AlleEntfernen`, `ZentraleZeigen`, Eigenschaften
  `NichtStoeren`, `Ungelesen`, `Anzahl`, Signale `Neu`, `Geaendert`,
  `Zaehler`; die Methoden aus 0.1.x bleiben. Beschreibung:
  [docs/dbus.md](docs/dbus.md).
- Bewusst **nicht** Teil dieses Programms: Töne, Einstellungsdialog.

Anwendungskennung: D-Bus-Name `org.freedesktop.Notifications` und
`org.querencia.Benachrichtigungen`, App-ID `org.querencia.Benachrichtigungen` ·
Binary: `querencia-benachrichtigungen`

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

Kritische Meldungen (`urgency` 2) erscheinen auch bei `anzeigen=nein` und
bei „Nicht stören“ als Popup (Freedesktop-Konvention: Akku- und
Sicherheitswarnungen sollen nicht ungesehen im Verlauf landen).

Der Dienst liest die Datei neu, sobald sie sich ändert (geprüft bei jeder
Meldung und beim Öffnen der Zentrale). Eine Seite in den Einstellungen
folgt später.

Gestartet wird der Dienst vom labwc-Autostart der Querencia-Sitzung
(Repository `querencia-shell`).

## Stand

| | |
|---|---|
| Version | 0.2.2 (Tag `benachrichtigungen/0.2.2`; dunkle Darstellung seit 0.2.1, Nebenschrift hell mit WCAG AA) |
| Letzte Prüfung | 21.10.2026: Unit-Tests einschließlich Kontrasttest hell und dunkel; isolierte GUI-Prüfung Popups und Zentrale hell und dunkel |
| Bekannte Lücken | siehe „Alpha“ und [docs/testing.md](docs/testing.md) |
| Getestet auf | Ubuntu 26.04, labwc 0.9.8, GTK 4.22, gtk4-layer-shell 1.3, Rust 1.93 |
| Nicht getestet | andere Compositoren und Distributionen, HiDPI, Mehrmonitor |

Änderungen: [CHANGELOG.md](CHANGELOG.md).

## Bauen

Voraussetzungen (Ubuntu 26.04):

```bash
sudo apt install build-essential pkg-config rustc cargo libgtk-4-dev libgtk4-layer-shell-dev
```

```bash
cargo build --release --locked
```

Das Ergebnis liegt unter `target/release/querencia-benachrichtigungen`.
Ausführliche Hinweise: [docs/building.md](docs/building.md).

## Prüfen

```bash
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
python3 werkzeug/i18n.py pruefen
```

Diese Befehle laufen ohne Bildschirm; die ersten drei sind auch die CI
(`.github/workflows/ci.yml`), die po-Prüfung braucht gettext und läuft lokal. Die **GUI-Prüfung** läuft in einer
abgeschotteten Sitzung (eigener labwc ohne Bildschirm, eigener D-Bus, eigenes
`XDG_RUNTIME_DIR` und XDG-Profil), niemals in der laufenden Desktopsitzung.
Sie braucht `cargo build --all-targets` (Panel-Attrappe) und die
Eingabehilfen `qeingabe`/`qausgabe` in `werkzeug/bin/`. Diese liegen nicht in
diesem Repo; Quelltext und Bauanleitung gleichnamiger Werkzeuge stehen in
`querencia-systemmonitor` (`werkzeug/qeingabe.c`, `werkzeug/qausgabe.c`, dort
docs/testing.md):

```bash
cargo build --all-targets --locked
bash werkzeug/gui-pruefung.sh
```

Fälle, Ablauf (auch die Zentrale: `szenario-zentrale.sh`) und Belege:
[docs/testing.md](docs/testing.md). Arbeitsweise: [CONTRIBUTING.md](CONTRIBUTING.md).

## Sprachen

Seit 0.1.2 sind die Texte, die der Dienst selbst zeigt, englisch im Code
(msgid) und deutsch in `po/de.po` (GNU gettext, Muster Querencia Editor
0.2.0). Das sind die Mitteilungszentrale, der Starter, der Hinweis und der
Tooltip „Close“/„Schließen“ im Popup. Betreff, Text und Aktionen kommen vom
Absender. Die Sprache folgt `LANGUAGE`/`LANG` (die
Sitzung setzt `LANGUAGE=de`); ohne deutschen Katalog erscheint Englisch.
Befehlszeile, `--hilfe`, Log, D-Bus-Fehlertexte und `GetServerInformation`
(„Querencia Benachrichtigungen“) bleiben unverändert.

## Vorbild und Abgrenzung

Vorbild ist **die Freedesktop-Spezifikation Desktop Notifications 1.2**; das
Aussehen folgt dem GNOME-2-Stil von Querencia (Symbolthema wie das
Querencia-Panel). Übernommen werden Bedienideen, **kein Quellcode, keine
Texte, keine Grafiken** anderer Benachrichtigungsdienste.

Abweichungen und Dinge, die bewusst fehlen:

- Popups über gtk4-layer-shell statt eigener Fenster; Position folgt der
  Exklusivzone des Panels.
- Keine Töne; der Verlauf liegt als JSON-Datei im Zustandsordner, nicht in
  einer Datenbank.

Fremdbestandteile: Rust-Abhängigkeiten (gtk4-rs, gtk4-layer-shell, zbus,
serde und Umfeld), Liste mit `cargo metadata --locked`; dynamisch gelinkt:
GTK 4, gtk4-layer-shell und ihre Systembibliotheken.

## Lizenz

Programm: GPL-3.0-or-later, siehe [LICENSE](LICENSE). Copyright 2026
Querencia Desktop contributors; Urheberhinweis in [COPYRIGHT](COPYRIGHT).
Eigener Code; aus notification-daemon wurde kein Code übernommen.

Eigene Vermerke: `werkzeug/virtual-keyboard-unstable-v1.xml` (gekürzte Kopie
des Wayland-Protokolls, Copyright Kristian Høgsberg u. a., MIT-artige Lizenz
im Dateikopf) und die mit `wayland-scanner` erzeugten Dateien unter
`werkzeug/bin/` (Lizenz der jeweiligen Protokolle); nur für das Prüfwerkzeug.

Name und Logo „Querencia“ sind keine freien Inhalte: [MARKE.md](MARKE.md).

## Fehler melden

Fehler, Wünsche und Fragen zu Querencia gehen an die gemeinsame Meldestelle:
<https://github.com/querencia-desktop/querencia/issues/new/choose>.
Sicherheitslücken bitte nicht öffentlich melden, siehe [SECURITY.md](SECURITY.md).
