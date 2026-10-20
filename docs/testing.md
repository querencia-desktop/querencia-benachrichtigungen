# Testen

Alles läuft **nur isoliert**: eigener headless labwc 0.9.8 mit eigenem
`XDG_RUNTIME_DIR` (`laufzeit/qed-runtime-*`) und eigenem `dbus-run-session`.
Der Sitzungsbus und der laufende Dienst der echten Sitzung werden nie
angesprochen.

```bash
cargo fmt --all --check
env -u WAYLAND_DISPLAY -u DISPLAY cargo clippy --all-targets --locked -- -D warnings
env -u WAYLAND_DISPLAY -u DISPLAY DBUS_SESSION_BUS_ADDRESS=disabled: cargo test --locked   # 37 Unit-Tests
python3 werkzeug/i18n.py pruefen          # .pot aktuell, de.po vollständig
```

Die Unit-Tests laufen ohne Katalog; `german_catalog_is_complete` prüft, dass
jede msgid in `po/de.po` übersetzt ist.

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

Weitere Diagnosen: `szenario-gross.sh` (übergroße Eingaben, `QB_KENNUNG`),
`szenario-caps.sh`, `szenario-icons.sh`, `szenario-aktion.sh`. Ergebnis in
`nachweis/gui/ergebnis.txt`, Bilder und `dienst.log` daneben. Die Läufe
überschreiben die eingecheckten Belege unter `nachweis/gui/`; danach
`git checkout -- nachweis/ && git clean -fq nachweis/gui/`.

Bekannt: `notify-send -A` scheitert auf Ubuntu an AppArmor
(`/etc/apparmor.d/notify-send` erlaubt kein `GetCapabilities`), nicht am
Dienst; Aktionen prüft `werkzeug/melden.py` (Gio).

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
