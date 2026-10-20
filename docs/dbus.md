# D-Bus-Schnittstellen

Querencia Benachrichtigungen belegt auf dem Sitzungsbus zwei Namen:

| Busname | Seit | Zweck |
|---|---|---|
| `org.freedesktop.Notifications` | 0.1.0 | Benachrichtigungen nach Freedesktop-Spezifikation 1.2 |
| `org.querencia.Benachrichtigungen` | 0.2.0 | Mitteilungszentrale, Verlauf, „Nicht stören“ (Panel-Applet) |

Beide Namen gehören demselben Prozess. Unter beiden liegt dasselbe Objekt
`/org/freedesktop/Notifications` mit beiden Schnittstellen. Der Dienst
belegt den Namen `org.freedesktop.Notifications` zuerst. Ist er schon
vergeben, beendet sich der Prozess mit Exit 1. Den Namen
`org.querencia.Benachrichtigungen` belegt er danach; schlägt das fehl, läuft
der Dienst trotzdem (nur ein Logeintrag). Eine D-Bus-Aktivierungsdatei gibt
es nicht. Gestartet wird der Dienst vom Autostart der Sitzung.

Für Clients gilt: den Namen `org.querencia.Benachrichtigungen` beobachten
(`NameOwnerChanged`). Hat er keinen Besitzer, läuft keine Zentrale. Dann
läuft entweder kein Dienst oder eine ältere Fassung (0.1.x) bzw. ein fremder
Dienst.

## org.freedesktop.Notifications

Nach Spezifikation 1.2: `GetCapabilities` (`actions`, `body`,
`body-markup`, `body-hyperlinks`, `icon-static`, `persistence`), `Notify`,
`CloseNotification`, `GetServerInformation` („Querencia
Benachrichtigungen“, „Querencia“, Version, „1.2“), Signale
`NotificationClosed` und `ActionInvoked`.

Ab 0.2.0 gilt außerdem:

- Jede Meldung außer solchen mit dem Hint `transient` kommt in den Verlauf.
- Ein Popup erscheint nur bei ausgeschaltetem „Nicht stören“ und mit
  `[Einblendung] anzeigen=ja` in der ini. Kritische Meldungen
  (`urgency` 2) erscheinen immer.
- Bei einer Meldung ohne Popup sendet der Dienst sofort
  `NotificationClosed(id, 1)`. So wartet ein Absender wie
  `notify-send --wait` nicht ewig.
- Klickt der Benutzer eine Aktion in der Zentrale an, sendet der Dienst
  ebenfalls `ActionInvoked(id, schlüssel)`, auch wenn das Popup schon zu ist.
  Nach der Spezifikation hört der Absender dann vielleicht nicht mehr zu.
  Nach einem Neustart des Dienstes bietet die Zentrale keine Aktionen mehr an.

## org.querencia.Benachrichtigungen

### Eintrag

Ein Eintrag des Verlaufs ist eine Struktur mit der Signatur
`(uxsssssysa(ss)b)`:

| Nr. | Typ | Feld | Bedeutung |
|---|---|---|---|
| 0 | `u` | id | ID der Meldung (wie bei `Notify`; nach einem Neustart vergibt der Dienst nur höhere IDs) |
| 1 | `x` | zeit | Unixzeit des Eintreffens in Sekunden (bei Ersatz durch `replaces_id`: der letzten Fassung) |
| 2 | `s` | app_name | Programmname des Absenders, kann leer sein |
| 3 | `s` | desktop_entry | Hint `desktop-entry`, kann leer sein |
| 4 | `s` | symbol | Hint `image-path`, sonst `app_icon`. Pfad, `file://`-URI oder Themensymbolname; kann leer sein (dann gilt das Symbol aus `desktop_entry`) |
| 5 | `s` | summary | Betreff |
| 6 | `s` | body | Text, kann Body-Markup enthalten (`b`, `i`, `u`, `a`) |
| 7 | `y` | dringlichkeit | 0 niedrig, 1 normal, 2 kritisch |
| 8 | `s` | kategorie | Hint `category`, kann leer sein |
| 9 | `a(ss)` | aktionen | Paare (Schlüssel, Beschriftung), `default` eingeschlossen |
| 10 | `b` | gelesen | ob der Eintrag gelesen ist |

Bilddaten aus `image-data` liefert die Schnittstelle nicht.

### Methoden

| Methode | Signatur | Wirkung |
|---|---|---|
| `Liste` | `() → a(uxsssssysa(ss)b)` | alle Einträge, neueste zuerst |
| `AlsGelesen` | `(au ids) → ()` | markiert `ids` als gelesen; ein leeres Feld markiert alle |
| `Entfernen` | `(au ids) → ()` | entfernt `ids` aus dem Verlauf; sichtbare Popups dazu schließen sich (Grund 2) |
| `AlleEntfernen` | `() → ()` | leert den Verlauf und schließt alle Popups (Grund 2) |
| `ZentraleZeigen` | `(s aktivierungs_token) → ()` | zeigt das Fenster der Mitteilungszentrale oder holt es nach vorn; Token aus xdg-activation oder leer |
| `ZentraleSchliessen` | `() → ()` | schließt das Fenster, falls es offen ist |

Unbekannte IDs werden ohne Fehler übergangen.

### Eigenschaften

| Eigenschaft | Typ | Zugriff | Bedeutung |
|---|---|---|---|
| `NichtStoeren` | `b` | lesen, schreiben | „Nicht stören“: Popups nur für kritische Meldungen; bleibt über Neustarts erhalten |
| `Ungelesen` | `u` | lesen | Zahl der ungelesenen Einträge (für den Zähler am Applet) |
| `Anzahl` | `u` | lesen | Zahl aller Einträge |
| `ZentraleOffen` | `b` | lesen | ob das Fenster der Zentrale offen ist |

Alle vier melden Änderungen über
`org.freedesktop.DBus.Properties.PropertiesChanged`.

### Signale

| Signal | Signatur | Wann |
|---|---|---|
| `Neu` | `((uxsssssysa(ss)b) eintrag)` | ein neuer Eintrag im Verlauf (nicht bei `transient`, nicht beim Ersatz) |
| `Geaendert` | `(au ids)` | Einträge geändert (gelesen, ersetzt) oder entfernt. Danach mit `Liste` neu lesen. Ein leeres Feld heißt: alles kann sich geändert haben |
| `Zaehler` | `(u ungelesen, u anzahl)` | nach jeder Änderung des Verlaufs |

### Bis 0.1.3 (bleibt erhalten)

| Name | Signatur | Bemerkung |
|---|---|---|
| `SetzeNichtStoeren` | `(b) → ()` | wie die Eigenschaft `NichtStoeren` |
| `Zustand` | `() → (buu)` | (nicht_stoeren, sichtbare Popups, wartende Meldungen) |
| `Verlauf` | `() → a(ussssyx)` | die letzten 50 Einträge, älteste zuerst: id, app_name, app_icon, summary, body, urgency, zeit |
| `VerlaufLoeschen` | `() → ()` | wie `AlleEntfernen` |
| Signal `ZustandGeaendert` | `(buu)` | nach „Nicht stören“ und nach Änderungen der Popup-Zahlen |
| Signal `VerlaufGeaendert` | `(u anzahl)` | nach jeder Änderung des Verlaufs |

## Beispiele

```bash
gdbus call --session --dest org.querencia.Benachrichtigungen \
    --object-path /org/freedesktop/Notifications \
    --method org.querencia.Benachrichtigungen.Liste
gdbus call --session --dest org.querencia.Benachrichtigungen \
    --object-path /org/freedesktop/Notifications \
    --method org.querencia.Benachrichtigungen.AlsGelesen "@au [3, 4]"
gdbus call --session --dest org.querencia.Benachrichtigungen \
    --object-path /org/freedesktop/Notifications \
    --method org.freedesktop.DBus.Properties.Get org.querencia.Benachrichtigungen Ungelesen
gdbus call --session --dest org.querencia.Benachrichtigungen \
    --object-path /org/freedesktop/Notifications \
    --method org.freedesktop.DBus.Properties.Set org.querencia.Benachrichtigungen NichtStoeren "<true>"
```

Für `gdbus` braucht ein Feld von IDs die Typangabe `@au`. Ohne sie
schickt `gdbus` ein `ai`, und der Aufruf scheitert an der Signatur.

## Panel-Applet (Vorschlag)

- Glocke mit Zähler `Ungelesen`; neu lesen bei `Zaehler` oder
  `PropertiesChanged`. Beim Start einmal `Get Ungelesen`.
- Klick: `ZentraleZeigen` mit dem xdg-activation-Token des Klicks (oder
  leer). Ist `ZentraleOffen` wahr, stattdessen `ZentraleSchliessen`.
- Kontextmenü: „Nicht stören“ als Häkchen über die Eigenschaft
  `NichtStoeren`, „Alle als gelesen markieren“ (`AlsGelesen []`).
- Hat `org.querencia.Benachrichtigungen` keinen Besitzer: Glocke
  ausgegraut, kein Zähler.
