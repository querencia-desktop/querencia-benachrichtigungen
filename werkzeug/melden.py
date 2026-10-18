#!/usr/bin/env python3
"""Kleiner Benachrichtigungs-Client für die Prüfung (Ersatz für notify-send -A,
dem Ubuntus AppArmor-Profil GetCapabilities verbietet).

  melden.py [-a schluessel=Beschriftung]... [-u low|normal|critical] [-t ms]
            [-i symbol] [-r id] [--bild BxH] [--wait] BETREFF [TEXT]

Gibt die ID aus; mit --wait wartet er auf ActionInvoked oder
NotificationClosed für diese ID und gibt den Aktionsschlüssel bzw.
"closed <grund>" aus.
"""
import sys
import gi
gi.require_version('Gio', '2.0')
gi.require_version('GLib', '2.0')
from gi.repository import Gio, GLib

args = sys.argv[1:]
actions, urgency, timeout, icon, replaces, wait, pos, bild = [], None, -1, "", 0, False, [], None
while args:
    a = args.pop(0)
    if a == '-a':
        k, _, l = args.pop(0).partition('=')
        actions += [k, l or k]
    elif a == '-u':
        urgency = {'low': 0, 'normal': 1, 'critical': 2}[args.pop(0)]
    elif a == '-t':
        timeout = int(args.pop(0))
    elif a == '-i':
        icon = args.pop(0)
    elif a == '-r':
        replaces = int(args.pop(0))
    elif a == '--wait':
        wait = True
    elif a == '--bild':
        bild = tuple(int(v) for v in args.pop(0).split('x'))
    else:
        pos.append(a)
summary = pos[0] if pos else ""
body = pos[1] if len(pos) > 1 else ""
hints = {}
if urgency is not None:
    hints['urgency'] = GLib.Variant('y', urgency)
if bild:
    # Hint image-data (iiibiiay): RGBA-Testbild mit Farbverlauf und Rahmen.
    w, h = bild
    daten = bytearray()
    for y in range(h):
        for x in range(w):
            rand = x < 8 or y < 8 or x >= w - 8 or y >= h - 8
            daten += bytes((240 if rand else x * 255 // max(w - 1, 1), 119 if rand else y * 255 // max(h - 1, 1), 70 if rand else 128, 255))
    hints['image-data'] = GLib.Variant('(iiibiiay)', (w, h, w * 4, True, 8, 4, bytes(daten)))

bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
loop = GLib.MainLoop()
result = {}


def on_signal(conn, sender, path, iface, signal, params):
    # Nur das erste Signal zur eigenen ID zählt (nach ActionInvoked folgt
    # sofort NotificationClosed, beide liegen oft in derselben Schleifenrunde).
    if params[0] != result.get('id') or result.get('fertig'):
        return
    result['fertig'] = True
    if signal == 'ActionInvoked':
        print(params[1])
    else:
        print(f"closed {params[1]}")
    loop.quit()


if wait:
    bus.signal_subscribe('org.freedesktop.Notifications', 'org.freedesktop.Notifications', None,
                         '/org/freedesktop/Notifications', None, Gio.DBusSignalFlags.NONE, on_signal)
reply = bus.call_sync('org.freedesktop.Notifications', '/org/freedesktop/Notifications',
                      'org.freedesktop.Notifications', 'Notify',
                      GLib.Variant('(susssasa{sv}i)', ('melden.py', replaces, icon, summary, body, actions, hints, timeout)),
                      GLib.VariantType('(u)'), Gio.DBusCallFlags.NONE, -1, None)
result['id'] = reply[0]
if wait:
    GLib.timeout_add_seconds(60, lambda: (print("timeout"), loop.quit()))
    loop.run()
else:
    print(reply[0])
