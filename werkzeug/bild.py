#!/usr/bin/env python3
"""Kleine PNG-Auswertung ohne Pillow (fehlt auf der VM) für die GUI-Prüfung.

  bild.py <png> groesse                 -> "B H"
  bild.py <png> pixel X Y               -> "R G B"
  bild.py <png> erste-helle-zeile X     -> y der ersten Zeile mit (fast) weißem Pixel in Spalte X, sonst -1
  bild.py <png> linke-kante Y           -> x der ersten nicht-hellen Spalte von rechts in Zeile Y
  bild.py <png> orange-rechts           -> Anzahl Pixel in Akzentfarbe (#F07746-nah) in den rechten 400 Spalten
"""
import struct
import sys
import zlib


def lesen(pfad):
    d = open(pfad, 'rb').read()
    pos = 8
    idat = b''
    w = h = ct = 0
    while pos < len(d):
        n, = struct.unpack('>I', d[pos:pos + 4])
        t = d[pos + 4:pos + 8]
        c = d[pos + 8:pos + 8 + n]
        pos += 12 + n
        if t == b'IHDR':
            w, h, _bd, ct = struct.unpack('>IIBB', c[:10])
        elif t == b'IDAT':
            idat += c
    raw = zlib.decompress(idat)
    bpp = {2: 3, 6: 4}[ct]
    st = w * bpp
    rows = []
    prev = bytearray(st)
    for y in range(h):
        f = raw[y * (st + 1)]
        line = bytearray(raw[y * (st + 1) + 1:(y + 1) * (st + 1)])
        for i in range(st):
            a = line[i - bpp] if i >= bpp else 0
            b = prev[i]
            c = prev[i - bpp] if i >= bpp else 0
            if f == 1:
                line[i] = (line[i] + a) & 255
            elif f == 2:
                line[i] = (line[i] + b) & 255
            elif f == 3:
                line[i] = (line[i] + (a + b) // 2) & 255
            elif f == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(bytes(line))
        prev = line
    return w, h, bpp, rows


def main():
    w, h, bpp, rows = lesen(sys.argv[1])
    px = lambda x, y: tuple(rows[y][x * bpp:x * bpp + 3])
    cmd = sys.argv[2]
    if cmd == 'groesse':
        print(w, h)
    elif cmd == 'pixel':
        print(*px(int(sys.argv[3]), int(sys.argv[4])))
    elif cmd == 'erste-helle-zeile':
        x = int(sys.argv[3])
        for y in range(h):
            r, g, b = px(x, y)
            if r > 240 and g > 240 and b > 240:
                print(y)
                return
        print(-1)
    elif cmd == 'linke-kante':
        y = int(sys.argv[3])
        for x in range(w - 1, -1, -1):
            r, g, b = px(x, y)
            if not (r > 200 and g > 200 and b > 200):
                print(x)
                return
        print(-1)
    elif cmd == 'orange-rechts':
        n = 0
        for y in range(h):
            for x in range(max(0, w - 400), w):
                r, g, b = px(x, y)
                if r > 220 and 90 < g < 150 and b < 110:
                    n += 1
        print(n)
    else:
        sys.exit("unbekannter Befehl")


main()
