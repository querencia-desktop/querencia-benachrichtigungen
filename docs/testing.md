# Testen

Alles läuft **nur isoliert**: eigener headless labwc 0.9.8 mit eigenem
`XDG_RUNTIME_DIR` (`laufzeit/qed-runtime-*`) und eigenem `dbus-run-session`.
Der Sitzungsbus und der laufende Dienst der echten Sitzung werden nie
angesprochen.

```bash
cargo fmt --all --check
env -u WAYLAND_DISPLAY -u DISPLAY cargo clippy --all-targets --locked -- -D warnings
env -u WAYLAND_DISPLAY -u DISPLAY DBUS_SESSION_BUS_ADDRESS=disabled: cargo test --locked   # 54 Unit-Tests
python3 werkzeug/i18n.py pruefen          # .pot aktuell, de.po vollständig
```

Die Unit-Tests laufen ohne Katalog; `german_catalog_is_complete` prüft, dass
jede msgid in `po/de.po` übersetzt ist. Ab 0.2.0 prüfen sie auch Verlauf
(Ringpuffer, gelesen, Ersatz, transient, JSON, Rechte 0600, defekte Datei),
ini (alle Schlüssel, ungültige Werte, Neulesen bei Änderung),
Anzeigedauer aus `dauer_s`, „einblenden“ (kritisch immer) und das
Verkleinern von `image-data` ohne GTK.

Die Prüfwerkzeuge `qeingabe` und `qausgabe` liegen gebaut unter
`werkzeug/bin/`; neu bauen:

```bash
cd werkzeug && gcc -O2 -o bin/qeingabe qeingabe.c bin/virtual-keyboard-unstable-v1-protocol.c \
    bin/wlr-virtual-pointer-unstable-v1-protocol.c -Ibin $(pkg-config --cflags --libs wayland-client xkbcommon) && \
    gcc -O2 -o bin/qausgabe qausgabe.c bin/wlr-output-management-unstable-v1-protocol.c -Ibin \
    $(pkg-config --cflags --libs wayland-client) && cd ..
```

`qeingabe`/`qausgabe` verweigern außerhalb eines `qed-runtime-*`-Ordners;
`qeingabe` rechnet Zeigerkoordinaten im 1280×720-Raster (die Szenarien
rechnen um).

**Regression** (42 Fälle, ca. 3 min; braucht `panel-attrappe` aus
`cargo build --all-targets`):

```bash
bash werkzeug/gui-pruefung.sh szenario.sh release
QB_BIN=<ordner>/querencia-benachrichtigungen bash werkzeug/gui-pruefung.sh szenario.sh release
```

**Sprache** (`szenario-sprache.sh`, ca. 30 s): Katalog eingeblendet
(`/proc/<pid>/maps`), Tooltip am Schließen-Knopf (Bild), Schließen per
Klick, kritische Meldung mit Aktionen:

```bash
QB_BIN=<ordner>/querencia-benachrichtigungen QB_LANGUAGE=de QB_GROESSE=1024x768 \
    bash werkzeug/gui-pruefung.sh szenario-sprache.sh release
QB_BIN=<ordner>/querencia-benachrichtigungen QB_LANGUAGE=en QB_GROESSE=1920x1080 \
    bash werkzeug/gui-pruefung.sh szenario-sprache.sh release
```

**Dunkle Darstellung (ab 0.2.1):** Die Popup-CSS hat einen Block
`@media (prefers-color-scheme: dark)`; der Unit-Test
`dunkelblock_deckt_jede_helle_farbe_ab` prüft, dass jede Regel mit fester
Farbe dort eine dunkle Fassung hat;
`programmname_und_hinweis_erreichen_aa` (ab 0.2.2) rechnet den Kontrast von
Programmname und Hinweis hell und dunkel nach (WCAG AA 4,5:1). Sichtbar prüfen lässt sich das nur mit
einem Settings-Portal im Wegwerf-Bus (der Harness hier schaltet Portale ab):
ein Ersatz, der `org.freedesktop.appearance color-scheme` (0/1) und
`org.gnome.desktop.interface` liefert und `SettingChanged` sendet, dazu ein
Verlauf mit festen Zeiten für pixelgleiche Bilder. Ohne
`org.gnome.desktop.interface` im Portal nimmt GTK 4.22 sein eingebautes
Thema statt `settings.ini`. Die Werkzeuge der Prüfung von 0.2.1 liegen bei
der Integration auf der Entwicklungs-VM (nicht im Repo).

Weitere Diagnosen: `szenario-gross.sh` (übergroße Eingaben, `QB_KENNUNG`),
`szenario-caps.sh`, `szenario-icons.sh`, `szenario-aktion.sh`. Ergebnis in
`nachweis/gui/ergebnis.txt`, Bilder und `dienst.log` daneben. Die Läufe
überschreiben die eingecheckten Belege unter `nachweis/gui/`; danach
`git checkout -- nachweis/ && git clean -fq nachweis/gui/`.

Bekannt: `notify-send -A` scheitert auf Ubuntu an AppArmor
(`/etc/apparmor.d/notify-send` erlaubt kein `GetCapabilities`), nicht am
Dienst; Aktionen prüft `werkzeug/melden.py` (Gio).

## Mitteilungszentrale (0.2.0)

`szenario-zentrale.sh` (ca. 2 min, 48 Fälle Z01–Z19): Namen, Introspektion,
Verlauf ohne `transient`, JSON (0600), Signale `Neu`/`Zaehler`/
`PropertiesChanged`, `--zentrale`, Tasten im Fenster (Entf, Eingabe →
`ActionInvoked(default)`, Tab + Leertaste → Aktionsknopf, Strg+F, Esc),
`AlsGelesen`, „Nicht stören“ (Eigenschaft, Zugriffstaste, alte Methode;
kritisch kommt durch), ini (`anzeigen`, `position` mit Pixelprüfung,
`dauer_s`, `hinweis`, `verlauf`), „Alle entfernen“ mit Rückfrage, Neustart
(Einträge, gelesen, IDs, keine Aktionen mehr), `--zentrale` ohne Dienst und
bei einem älteren Dienst (`QB_ALT`, z. B. die 0.1.3-Ablage).

```bash
QB_BIN=<ordner>/querencia-benachrichtigungen QB_THEMA=1 QB_EIGENES_HOME=1 \
    QB_ALT=~/opt/querencia/benachrichtigungen/d8287021dd7616e2/querencia-benachrichtigungen \
    QB_LANGUAGE=de QB_GROESSE=1024x768 QB_ZEITLIMIT=300 \
    bash werkzeug/gui-pruefung.sh szenario-zentrale.sh release
```

`QB_THEMA=1` kopiert GTK-Thema Querencia-Klassisch und Symbolthema Querencia
(nur gelesen) ins Profil und setzt sie wie live; `QB_EIGENES_HOME=1` gibt dem
Lauf ein eigenes HOME im Profil. Bilder `z01…z14-<sprache>-<größe>.png`;
mit `QB_DEBUG=1` schreibt der Dienst „Zentrale: n von m Einträgen
angezeigt“ ins Log (die Suche prüft das). `werkzeug/melden.py` kann jetzt
auch `-n programm`, `--desktop id` und `--transient`; `bild.py popup-pixel`
zählt Popup-Pixel in einem Rechteck.

## Ohne Wayland, ohne Layer-Shell (0.1.3, G2-AI, QD-062)

```bash
bash werkzeug/pruefung-g2ai.sh <binary> <ausgabeordner>     # N01–N04, eigener Bus, headless labwc
```

| Fall | Erwartung |
|---|---|
| N01 | Ohne Compositor: Exit 1, Name frei. |
| N02 | Nur X11 (Xwayland, kein Wayland-Socket): kein Rückfall, Exit 1, Name frei. |
| N03 | Mit Wayland: bereit, der Name gehört dem Dienst. |
| N04 | labwc beendet: Der Dienst endet sofort, der Name ist frei. |
