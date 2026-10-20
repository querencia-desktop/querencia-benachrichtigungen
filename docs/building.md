# Bauen

Ubuntu 26.04: `cargo`, `rustc` (≥ 1.92, Ubuntu 1.93.1), `pkgconf`,
`libgtk-4-dev`, `libgtk4-layer-shell-dev`. Bau aus dem lokalen Crate-Cache:

```bash
cargo build --release --all-targets --locked --offline
```

`target/release/querencia-benachrichtigungen` ist der Dienst,
`target/release/panel-attrappe` nur ein Prüfwerkzeug (oberes Panel mit
Exklusivzone für die isolierte GUI-Prüfung). Für Pakete und Anhänge mit
`RUSTFLAGS="--remap-path-prefix=$HOME=~"` bauen, damit keine Pfade des
Bauordners in der Binary stehen.

**Übersetzungen (ab 0.1.2):** Der Dienst sucht
`<lang>/LC_MESSAGES/querencia-benachrichtigungen.mo` in
`<Ordner der Binary>/share/locale`, dann `<Ordner der Binary>/../share/locale`,
zuletzt `/usr/share/locale` (`QUERENCIA_BENACHRICHTIGUNGEN_LOCALEDIR` setzt den
Ordner ausdrücklich). Den Katalog erzeugt GNU gettext oder das Werkzeug des
Repos:

```bash
msgfmt -c -o share/locale/de/LC_MESSAGES/querencia-benachrichtigungen.mo po/de.po
python3 werkzeug/i18n.py mo <ziel>     # <ziel>/de/LC_MESSAGES/querencia-benachrichtigungen.mo
```

Ohne Katalog ist der Tooltip englisch.

**Start in der Sitzung:** Der labwc-Autostart der Querencia-Sitzung
(`querencia-runtime`, `session/querencia/labwc/autostart`) startet
`~/opt/querencia/bin/querencia-benachrichtigungen`, wenn der Link ausführbar
ist. Ist `org.freedesktop.Notifications` schon belegt, beendet sich ein
zweiter Dienst selbst (Exit 1).

**Integration (nur auf der Entwicklungs-VM, nicht Teil der Veröffentlichung):**
Skripte unter `integration/` stellen die unveränderliche Ablage
`~/opt/querencia/benachrichtigungen/<sha16>/` (mit `share/locale` daneben) und
den Link um (`pruefen|aktivieren|zurueck`). Sie starten und beenden keinen
Dienst; wirksam wird ein Wechsel mit dem nächsten Sitzungsstart. Der Ordner
enthält feste Pfade der VM und wird vor der Veröffentlichung herausgefiltert
(ENTSCHEIDUNGEN Nr. 13).
