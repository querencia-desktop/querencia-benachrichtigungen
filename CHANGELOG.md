# Änderungen

## 0.1.2 – 2026-10-20

Englisch und Deutsch über GNU gettext (G2-AH, Muster Querencia Editor 0.2.0):

- Der einzige eigene sichtbare Text, der Tooltip am Schließen-Knopf der
  Popups, ist englisch im Code („Close“), Deutsch aus `po/de.po` mit dem
  bisherigen Wortlaut („Schließen“). Dienst, Popups, Schnittstellen und
  Befehlszeile sonst unverändert.
- `werkzeug/i18n.py` für die po-Pflege; GUI-Prüfung mit `QB_LANGUAGE`,
  `QB_GROESSE` und neuem Szenario `szenario-sprache.sh`; Quelltexte von
  `qeingabe`/`qausgabe` liegen jetzt im Repo (bauen byte-gleich nach).
- Doku nach ENTSCHEIDUNGEN Nr. 17 (`docs/building.md`, `docs/testing.md`,
  Arbeitsregeln), `LICENSE` mit Lizenzkopf
  (Nr. 14 Q5), dieses Änderungsprotokoll.

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
