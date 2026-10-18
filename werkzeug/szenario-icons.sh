#!/bin/bash
# Diagnose: Welche Datei liefert GTK 4 für Symbolnamen im Standardthema?
set -u
. "$HMP04_PROFILE/env.sh"
mkdir -p "$OUT"
python3 - >"$OUT/icons-diagnose.txt" 2>&1 <<'PY'
import gi
gi.require_version('Gtk', '4.0')
from gi.repository import Gtk, Gdk
Gtk.init()
d = Gdk.Display.get_default()
t = Gtk.IconTheme.get_for_display(d)
print("Thema:", t.get_theme_name())
print("Suchpfad:", t.get_search_path())
for name in ["dialog-warning", "dialog-information", "dialog-error", "mail-unread", "window-close-symbolic", "image-missing", "org.querencia.Editor", "firefox", "utilities-terminal"]:
    p = t.lookup_icon(name, None, 48, 1, Gtk.TextDirection.LTR, 0)
    f = p.get_file()
    print(f"{name:24} has={t.has_icon(name)} icon_name={p.get_icon_name()} symbolic={p.is_symbolic()} file={f.get_path() if f else None}")
for thema in ["gnome", "mate", "Humanity"]:
    t2 = Gtk.IconTheme.new(); t2.set_theme_name(thema)
    p = t2.lookup_icon("dialog-warning", None, 48, 1, Gtk.TextDirection.LTR, 0)
    f = p.get_file()
    print(f"Thema {thema}: dialog-warning -> {f.get_path() if f else None}")
PY
touch "$HMP04_PROFILE/logs/FERTIG"
