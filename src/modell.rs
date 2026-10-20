//! Datenmodell des Dienstes: Meldung mit Hints, Dringlichkeit, Anzeigedauer
//! und der von D-Bus- und GTK-Seite gemeinsam benutzte Zustand (mit Verlauf
//! und Einstellungen). Alles hier läuft ohne Display und ist in Unit-Tests
//! prüfbar.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::time::Duration;

use zbus::zvariant::{OwnedValue, Value};

use crate::einstellung::{self, Einstellungen};
use crate::verlauf::Verlauf;

/// Dringlichkeit nach Spezifikation (Hint `urgency`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dringlichkeit {
    Niedrig,
    Normal,
    Kritisch,
}

impl Dringlichkeit {
    pub fn aus_byte(b: u8) -> Self {
        match b {
            0 => Self::Niedrig,
            2 => Self::Kritisch,
            _ => Self::Normal,
        }
    }

    pub fn als_byte(self) -> u8 {
        match self {
            Self::Niedrig => 0,
            Self::Normal => 1,
            Self::Kritisch => 2,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Niedrig => "niedrig",
            Self::Normal => "normal",
            Self::Kritisch => "kritisch",
        }
    }
}

/// Rohbild aus dem Hint `image-data` (iiibiiay).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BildDaten {
    pub breite: i32,
    pub hoehe: i32,
    pub zeilenlaenge: i32,
    pub alpha: bool,
    pub bits_je_kanal: i32,
    pub kanaele: i32,
    pub daten: Vec<u8>,
}

impl BildDaten {
    /// Prüft, ob die Maße zum Puffer passen und GTK sie darstellen kann.
    pub fn plausibel(&self) -> bool {
        if self.bits_je_kanal != 8
            || self.breite <= 0
            || self.hoehe <= 0
            || self.breite > 4096
            || self.hoehe > 4096
        {
            return false;
        }
        let kanaele = if self.alpha { 4 } else { 3 };
        if self.kanaele != kanaele || self.zeilenlaenge < self.breite * kanaele {
            return false;
        }
        let noetig = (self.hoehe as usize - 1) * self.zeilenlaenge as usize
            + (self.breite * kanaele) as usize;
        self.daten.len() >= noetig
    }

    /// Verkleinert auf höchstens `max` × `max` (Seitenverhältnis bleibt,
    /// Mittelwert je Zielpixel), ohne GTK. Kleinere Bilder bleiben, wie sie
    /// sind. Für den Verlauf, damit große `image-data` (Electron schickt
    /// 512 × 512) nicht 200-mal im Speicher liegen.
    pub fn verkleinert(&self, max: i32) -> Option<Self> {
        if !self.plausibel() || max <= 0 {
            return None;
        }
        if self.breite <= max && self.hoehe <= max {
            return Some(self.clone());
        }
        let (zb, zh) = if self.breite >= self.hoehe {
            (
                max,
                (self.hoehe as i64 * max as i64 / self.breite as i64).max(1) as i32,
            )
        } else {
            (
                (self.breite as i64 * max as i64 / self.hoehe as i64).max(1) as i32,
                max,
            )
        };
        let k = self.kanaele as usize;
        let mut daten = Vec::with_capacity((zb * zh) as usize * k);
        for y in 0..zh {
            let y0 = (y as i64 * self.hoehe as i64 / zh as i64) as usize;
            let y1 = (((y + 1) as i64 * self.hoehe as i64 / zh as i64) as usize).max(y0 + 1);
            for x in 0..zb {
                let x0 = (x as i64 * self.breite as i64 / zb as i64) as usize;
                let x1 = (((x + 1) as i64 * self.breite as i64 / zb as i64) as usize).max(x0 + 1);
                let mut summe = [0u64; 4];
                for yy in y0..y1 {
                    let zeile = yy * self.zeilenlaenge as usize;
                    for xx in x0..x1 {
                        let p = zeile + xx * k;
                        for (c, s) in summe.iter_mut().enumerate().take(k) {
                            *s += self.daten[p + c] as u64;
                        }
                    }
                }
                let n = ((y1 - y0) * (x1 - x0)) as u64;
                daten.extend(summe.iter().take(k).map(|s| (s / n) as u8));
            }
        }
        Some(Self {
            breite: zb,
            hoehe: zh,
            zeilenlaenge: zb * self.kanaele,
            alpha: self.alpha,
            bits_je_kanal: 8,
            kanaele: self.kanaele,
            daten,
        })
    }
}

/// Eine Benachrichtigung, wie sie über `Notify` ankam, mit ausgewerteten Hints.
#[derive(Clone, Debug)]
pub struct Meldung {
    pub id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    /// Paare (Schlüssel, Beschriftung); `default` ist die Klick-Aktion.
    pub actions: Vec<(String, String)>,
    pub dringlichkeit: Dringlichkeit,
    pub bild_pfad: Option<String>,
    pub bild_daten: Option<BildDaten>,
    pub desktop_entry: Option<String>,
    pub transient: bool,
    pub resident: bool,
    pub category: Option<String>,
    pub expire_timeout: i32,
    /// Unixzeit des Eintreffens in Sekunden.
    pub zeit: i64,
}

impl Meldung {
    /// Baut die Meldung aus den `Notify`-Argumenten.
    #[allow(clippy::too_many_arguments)]
    pub fn neu(
        id: u32,
        app_name: String,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: &HashMap<String, OwnedValue>,
        expire_timeout: i32,
        zeit: i64,
    ) -> Self {
        Self {
            id,
            app_name,
            app_icon,
            summary,
            body,
            actions: aktionen_paare(actions),
            dringlichkeit: hint_dringlichkeit(hints),
            bild_pfad: hint_string(hints, &["image-path", "image_path"]),
            bild_daten: hint_bild(hints),
            desktop_entry: hint_string(hints, &["desktop-entry"]),
            transient: hint_bool(hints, "transient"),
            resident: hint_bool(hints, "resident"),
            category: hint_string(hints, &["category"]),
            expire_timeout,
            zeit,
        }
    }

    pub fn hat_default_aktion(&self) -> bool {
        self.actions.iter().any(|(k, _)| k == "default")
    }

    /// Aktionen, die als Knöpfe erscheinen (alle außer `default`).
    pub fn knopf_aktionen(&self) -> impl Iterator<Item = &(String, String)> {
        self.actions.iter().filter(|(k, _)| k != "default")
    }

    /// Wie lange das Popup steht; `None` heißt: bis es geschlossen wird.
    /// `dauer_s` ist die Standarddauer normaler Meldungen aus der ini.
    pub fn anzeigedauer(&self, dauer_s: u32) -> Option<Duration> {
        anzeigedauer(self.dringlichkeit, self.expire_timeout, dauer_s)
    }
}

/// Anzeigedauer nach Dringlichkeit und `expire_timeout` (Millisekunden):
/// kritische Meldungen bleiben immer stehen; 0 heißt „nie ablaufen“; ein
/// positiver Wert gilt; -1 (oder jeder andere negative Wert) nimmt den
/// Standard: `dauer_s` bei normaler, drei Fünftel davon (mindestens 1 s) bei
/// niedriger Dringlichkeit – mit der Vorgabe 5 s also 5 und 3 s wie bis
/// 0.1.3. `dauer_s` = 0 heißt auch hier „bis geschlossen“.
pub fn anzeigedauer(
    dringlichkeit: Dringlichkeit,
    expire_timeout: i32,
    dauer_s: u32,
) -> Option<Duration> {
    match dringlichkeit {
        Dringlichkeit::Kritisch => None,
        _ if expire_timeout == 0 => None,
        _ if expire_timeout > 0 => Some(Duration::from_millis(expire_timeout as u64)),
        _ if dauer_s == 0 => None,
        Dringlichkeit::Niedrig => Some(Duration::from_secs(((dauer_s * 3 + 2) / 5).max(1) as u64)),
        Dringlichkeit::Normal => Some(Duration::from_secs(dauer_s as u64)),
    }
}

/// Ob eine Meldung als Popup erscheint: kritische immer, sonst nur ohne
/// „Nicht stören“ und mit `[Einblendung] anzeigen=ja`. In den Verlauf kommt
/// sie in jedem Fall (außer `transient`).
pub fn einblenden(dringlichkeit: Dringlichkeit, nicht_stoeren: bool, e: &Einstellungen) -> bool {
    dringlichkeit == Dringlichkeit::Kritisch || (!nicht_stoeren && e.anzeigen)
}

/// Die flache Aktionsliste der Spezifikation in Paare; ein überzähliges
/// letztes Element wird verworfen.
pub fn aktionen_paare(actions: Vec<String>) -> Vec<(String, String)> {
    let mut paare = Vec::with_capacity(actions.len() / 2);
    let mut it = actions.into_iter();
    while let (Some(k), Some(l)) = (it.next(), it.next()) {
        paare.push((k, l));
    }
    paare
}

fn hint_dringlichkeit(hints: &HashMap<String, OwnedValue>) -> Dringlichkeit {
    match hints.get("urgency").map(|v| &**v) {
        Some(Value::U8(b)) => Dringlichkeit::aus_byte(*b),
        Some(Value::U32(n)) => Dringlichkeit::aus_byte((*n).min(255) as u8),
        Some(Value::I32(n)) => Dringlichkeit::aus_byte((*n).clamp(0, 255) as u8),
        Some(Value::U16(n)) => Dringlichkeit::aus_byte((*n).min(255) as u8),
        Some(Value::I16(n)) => Dringlichkeit::aus_byte((*n).clamp(0, 255) as u8),
        Some(Value::I64(n)) => Dringlichkeit::aus_byte((*n).clamp(0, 255) as u8),
        Some(Value::U64(n)) => Dringlichkeit::aus_byte((*n).min(255) as u8),
        _ => Dringlichkeit::Normal,
    }
}

fn hint_string(hints: &HashMap<String, OwnedValue>, namen: &[&str]) -> Option<String> {
    namen
        .iter()
        .find_map(|n| match hints.get(*n).map(|v| &**v) {
            Some(Value::Str(s)) if !s.is_empty() => Some(s.to_string()),
            _ => None,
        })
}

fn hint_bool(hints: &HashMap<String, OwnedValue>, name: &str) -> bool {
    match hints.get(name).map(|v| &**v) {
        Some(Value::Bool(b)) => *b,
        Some(Value::U8(n)) => *n != 0,
        Some(Value::I32(n)) => *n != 0,
        Some(Value::U32(n)) => *n != 0,
        _ => false,
    }
}

/// `image-data` (Spez. 1.2), `image_data` (1.1) oder `icon_data` (älter).
fn hint_bild(hints: &HashMap<String, OwnedValue>) -> Option<BildDaten> {
    ["image-data", "image_data", "icon_data"]
        .iter()
        .find_map(|n| hints.get(*n))
        .and_then(|v| bild_aus_wert(v))
}

fn bild_aus_wert(v: &Value<'_>) -> Option<BildDaten> {
    let Value::Structure(s) = v else { return None };
    let f = s.fields();
    if f.len() != 7 {
        return None;
    }
    let ganz = |v: &Value<'_>| match v {
        Value::I32(n) => Some(*n),
        _ => None,
    };
    let Value::Bool(alpha) = &f[3] else {
        return None;
    };
    let Value::Array(a) = &f[6] else { return None };
    let mut daten = Vec::with_capacity(a.len());
    for e in a.iter() {
        match e {
            Value::U8(b) => daten.push(*b),
            _ => return None,
        }
    }
    let bild = BildDaten {
        breite: ganz(&f[0])?,
        hoehe: ganz(&f[1])?,
        zeilenlaenge: ganz(&f[2])?,
        alpha: *alpha,
        bits_je_kanal: ganz(&f[4])?,
        kanaele: ganz(&f[5])?,
        daten,
    };
    bild.plausibel().then_some(bild)
}

/// Gemeinsamer Zustand von D-Bus-Seite (zbus-Ausführer) und GTK-Seite.
#[derive(Debug)]
pub struct Zustand {
    naechste_id: u32,
    pub nicht_stoeren: bool,
    pub verlauf: Verlauf,
    /// IDs, die sichtbar sind oder warten (für `CloseNotification`).
    pub aktiv: BTreeSet<u32>,
    pub sichtbar: u32,
    pub wartend: u32,
    /// Datei, deren Vorhandensein „Nicht stören“ über Neustarts hält.
    pub nicht_stoeren_datei: Option<PathBuf>,
    /// JSON-Datei des Verlaufs (`None`: nur im Speicher, Tests).
    pub verlauf_datei: Option<PathBuf>,
    pub einstellungen: einstellung::Quelle,
    /// Ob das Fenster der Mitteilungszentrale gerade offen ist.
    pub zentrale_offen: bool,
}

/// Ablageorte des Zustands; `None` heißt jeweils: nur im Speicher.
#[derive(Debug, Default)]
pub struct Orte {
    pub nicht_stoeren: Option<PathBuf>,
    pub verlauf: Option<PathBuf>,
    pub ini: Option<PathBuf>,
}

impl Zustand {
    /// Liest „Nicht stören“, Einstellungen und Verlauf. Die Texte im zweiten
    /// Wert sind fürs Log.
    pub fn neu(orte: Orte) -> (Self, Vec<String>) {
        let nicht_stoeren = orte.nicht_stoeren.as_ref().is_some_and(|p| p.exists());
        let einstellungen = einstellung::Quelle::neu(orte.ini);
        let mut log = Vec::new();
        let verlauf = match &orte.verlauf {
            Some(p) => {
                let (v, hinweis) = Verlauf::laden(p, einstellungen.werte.verlauf);
                log.extend(hinweis);
                log.push(format!(
                    "Verlauf {}: {} Einträge, {} ungelesen (höchstens {})",
                    p.display(),
                    v.len(),
                    v.ungelesen(),
                    v.kapazitaet()
                ));
                v
            }
            None => Verlauf::neu(einstellungen.werte.verlauf),
        };
        // IDs aus dem gespeicherten Verlauf nicht wieder vergeben.
        let naechste_id = verlauf
            .hoechste_id()
            .checked_add(1)
            .filter(|&n| n != 0)
            .unwrap_or(1);
        (
            Self {
                naechste_id,
                nicht_stoeren,
                verlauf,
                aktiv: BTreeSet::new(),
                sichtbar: 0,
                wartend: 0,
                nicht_stoeren_datei: orte.nicht_stoeren,
                verlauf_datei: orte.verlauf,
                einstellungen,
                zentrale_offen: false,
            },
            log,
        )
    }

    pub fn ein(&self) -> &Einstellungen {
        &self.einstellungen.werte
    }

    /// Liest die ini neu, falls sie sich geändert hat, und passt die Größe
    /// des Verlaufs an. Liefert Texte fürs Log.
    pub fn einstellungen_auffrischen(&mut self) -> Vec<String> {
        let log = self.einstellungen.auffrischen();
        if !log.is_empty()
            && self
                .verlauf
                .setze_kapazitaet(self.einstellungen.werte.verlauf)
        {
            self.verlauf_sichern();
        }
        log
    }

    /// Schreibt den Verlauf in seine Datei (Fehler nur ins Log).
    pub fn verlauf_sichern(&self) {
        if let Some(p) = &self.verlauf_datei
            && let Err(e) = self.verlauf.speichern(p)
        {
            eprintln!(
                "querencia-benachrichtigungen: Verlauf nicht gespeichert ({}): {e}",
                p.display()
            );
        }
    }

    /// Nächste freie ID; 0 wird nie vergeben, nach u32::MAX geht es bei 1 weiter.
    pub fn neue_id(&mut self) -> u32 {
        let id = self.naechste_id;
        self.naechste_id = if id == u32::MAX { 1 } else { id + 1 };
        id
    }

    /// Setzt „Nicht stören“ und schreibt es auf die Platte. Liefert, ob sich
    /// etwas geändert hat.
    pub fn setze_nicht_stoeren(&mut self, an: bool) -> bool {
        if self.nicht_stoeren == an {
            return false;
        }
        self.nicht_stoeren = an;
        if let Some(p) = &self.nicht_stoeren_datei {
            let ergebnis = if an {
                p.parent()
                    .map(std::fs::create_dir_all)
                    .transpose()
                    .and_then(|_| std::fs::write(p, b"an\n"))
            } else {
                match std::fs::remove_file(p) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                    _ => Ok(()),
                }
            };
            if let Err(e) = ergebnis {
                eprintln!(
                    "querencia-benachrichtigungen: Nicht-stören-Zustand nicht gespeichert ({}): {e}",
                    p.display()
                );
            }
        }
        true
    }
}

pub fn jetzt() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::{Array, Structure, StructureBuilder};

    fn ov<T: Into<Value<'static>>>(t: T) -> OwnedValue {
        OwnedValue::try_from(t.into()).expect("OwnedValue")
    }

    fn meldung(id: u32, dringlichkeit: Dringlichkeit, timeout: i32) -> Meldung {
        let mut hints = HashMap::new();
        hints.insert("urgency".to_string(), ov(dringlichkeit.als_byte()));
        Meldung::neu(
            id,
            "app".into(),
            String::new(),
            format!("Betreff {id}"),
            "Text".into(),
            vec![],
            &hints,
            timeout,
            1_000 + id as i64,
        )
    }

    #[test]
    fn anzeigedauer_standardwerte() {
        assert_eq!(
            anzeigedauer(Dringlichkeit::Normal, -1, 5),
            Some(Duration::from_secs(5))
        );
        assert_eq!(
            anzeigedauer(Dringlichkeit::Niedrig, -1, 5),
            Some(Duration::from_secs(3))
        );
        assert_eq!(anzeigedauer(Dringlichkeit::Kritisch, -1, 5), None);
    }

    #[test]
    fn anzeigedauer_explizit() {
        assert_eq!(
            anzeigedauer(Dringlichkeit::Normal, 1500, 5),
            Some(Duration::from_millis(1500))
        );
        assert_eq!(
            anzeigedauer(Dringlichkeit::Niedrig, 10_000, 5),
            Some(Duration::from_secs(10))
        );
        assert_eq!(
            anzeigedauer(Dringlichkeit::Normal, 0, 5),
            None,
            "0 = nie ablaufen"
        );
        assert_eq!(
            anzeigedauer(Dringlichkeit::Kritisch, 2000, 5),
            None,
            "kritisch bleibt stehen"
        );
        assert_eq!(
            anzeigedauer(Dringlichkeit::Normal, -7, 5),
            Some(Duration::from_secs(5)),
            "negativ = Standard"
        );
    }

    #[test]
    fn dringlichkeit_aus_hints_verschiedener_typen() {
        for (wert, soll) in [
            (OwnedValue::from(0u8), Dringlichkeit::Niedrig),
            (OwnedValue::from(1u8), Dringlichkeit::Normal),
            (OwnedValue::from(2u8), Dringlichkeit::Kritisch),
            (OwnedValue::from(2u32), Dringlichkeit::Kritisch),
            (OwnedValue::from(2i32), Dringlichkeit::Kritisch),
            (OwnedValue::from(9u8), Dringlichkeit::Normal),
            (ov("kritisch"), Dringlichkeit::Normal),
        ] {
            let mut hints = HashMap::new();
            hints.insert("urgency".to_string(), wert);
            assert_eq!(hint_dringlichkeit(&hints), soll);
        }
        assert_eq!(hint_dringlichkeit(&HashMap::new()), Dringlichkeit::Normal);
    }

    #[test]
    fn hints_strings_und_bools() {
        let mut hints = HashMap::new();
        hints.insert("image-path".to_string(), ov("/tmp/x.png"));
        hints.insert("desktop-entry".to_string(), ov("org.querencia.Editor"));
        hints.insert("transient".to_string(), OwnedValue::from(true));
        hints.insert("category".to_string(), ov("email.arrived"));
        let m = Meldung::neu(
            1,
            "a".into(),
            "".into(),
            "s".into(),
            "b".into(),
            vec![],
            &hints,
            -1,
            0,
        );
        assert_eq!(m.bild_pfad.as_deref(), Some("/tmp/x.png"));
        assert_eq!(m.desktop_entry.as_deref(), Some("org.querencia.Editor"));
        assert!(m.transient);
        assert!(!m.resident);
        assert_eq!(m.category.as_deref(), Some("email.arrived"));
    }

    fn bildwert(
        b: i32,
        h: i32,
        zl: i32,
        alpha: bool,
        bps: i32,
        kan: i32,
        daten: Vec<u8>,
    ) -> OwnedValue {
        let s: Structure = StructureBuilder::new()
            .add_field(b)
            .add_field(h)
            .add_field(zl)
            .add_field(alpha)
            .add_field(bps)
            .add_field(kan)
            .append_field(Value::Array(Array::from(daten)))
            .build()
            .expect("Struktur");
        OwnedValue::try_from(Value::Structure(s)).expect("OwnedValue")
    }

    #[test]
    fn bilddaten_werden_gelesen_und_geprueft() {
        let mut hints = HashMap::new();
        hints.insert(
            "image-data".to_string(),
            bildwert(2, 2, 8, true, 8, 4, vec![7; 16]),
        );
        let b = hint_bild(&hints).expect("Bild");
        assert_eq!(
            (b.breite, b.hoehe, b.zeilenlaenge, b.alpha, b.kanaele),
            (2, 2, 8, true, 4)
        );
        assert_eq!(b.daten.len(), 16);
        // zu kurzer Puffer
        hints.insert(
            "image-data".to_string(),
            bildwert(2, 2, 8, true, 8, 4, vec![7; 10]),
        );
        assert!(hint_bild(&hints).is_none());
        // 16 Bit je Kanal kann GTK so nicht
        hints.insert(
            "image-data".to_string(),
            bildwert(2, 2, 8, false, 16, 3, vec![7; 32]),
        );
        assert!(hint_bild(&hints).is_none());
        // alter Name
        hints.clear();
        hints.insert(
            "icon_data".to_string(),
            bildwert(1, 1, 3, false, 8, 3, vec![1, 2, 3]),
        );
        assert!(hint_bild(&hints).is_some());
    }

    #[test]
    fn aktionen_paare_und_default() {
        let m = Meldung::neu(
            1,
            "a".into(),
            "".into(),
            "s".into(),
            "b".into(),
            vec![
                "default".into(),
                "Öffnen".into(),
                "ja".into(),
                "Ja".into(),
                "einsam".into(),
            ],
            &HashMap::new(),
            -1,
            0,
        );
        assert_eq!(m.actions.len(), 2);
        assert!(m.hat_default_aktion());
        let knoepfe: Vec<_> = m.knopf_aktionen().collect();
        assert_eq!(knoepfe, vec![&("ja".to_string(), "Ja".to_string())]);
    }

    #[test]
    fn anzeigedauer_aus_der_ini() {
        assert_eq!(
            anzeigedauer(Dringlichkeit::Normal, -1, 10),
            Some(Duration::from_secs(10))
        );
        assert_eq!(
            anzeigedauer(Dringlichkeit::Niedrig, -1, 10),
            Some(Duration::from_secs(6))
        );
        assert_eq!(
            anzeigedauer(Dringlichkeit::Niedrig, -1, 1),
            Some(Duration::from_secs(1))
        );
        assert_eq!(
            anzeigedauer(Dringlichkeit::Normal, -1, 0),
            None,
            "0 = bis geschlossen"
        );
        assert_eq!(
            anzeigedauer(Dringlichkeit::Normal, 1500, 0),
            Some(Duration::from_millis(1500)),
            "Absender gewinnt"
        );
    }

    #[test]
    fn einblenden_nach_nicht_stoeren_und_ini() {
        let an = Einstellungen::default();
        let aus = Einstellungen {
            anzeigen: false,
            ..Einstellungen::default()
        };
        assert!(einblenden(Dringlichkeit::Normal, false, &an));
        assert!(!einblenden(Dringlichkeit::Normal, true, &an));
        assert!(!einblenden(Dringlichkeit::Niedrig, false, &aus));
        // kritisch immer
        assert!(einblenden(Dringlichkeit::Kritisch, true, &an));
        assert!(einblenden(Dringlichkeit::Kritisch, true, &aus));
    }

    #[test]
    fn bild_verkleinern_ohne_gtk() {
        let gross = BildDaten {
            breite: 512,
            hoehe: 256,
            zeilenlaenge: 512 * 4,
            alpha: true,
            bits_je_kanal: 8,
            kanaele: 4,
            daten: vec![200; 512 * 256 * 4],
        };
        let k = gross.verkleinert(48).expect("Bild");
        assert_eq!((k.breite, k.hoehe, k.zeilenlaenge), (48, 24, 192));
        assert_eq!(k.daten.len(), 48 * 24 * 4);
        assert!(k.plausibel());
        assert!(k.daten.iter().all(|&b| b == 200));
        // RGB mit Zeilenpolster, hochkant
        let mut daten = vec![0u8; 100 * 304];
        for z in daten.chunks_mut(304) {
            z[..300].fill(10);
        }
        let rgb = BildDaten {
            breite: 100,
            hoehe: 100,
            zeilenlaenge: 304,
            alpha: false,
            bits_je_kanal: 8,
            kanaele: 3,
            daten,
        };
        let k = rgb.verkleinert(30).expect("Bild");
        assert_eq!((k.breite, k.hoehe, k.kanaele), (30, 30, 3));
        assert!(
            k.daten.iter().all(|&b| b == 10),
            "Polster nicht mitgemittelt"
        );
        let klein = rgb.verkleinert(200).expect("Bild");
        assert_eq!(klein, rgb, "kleinere bleiben");
    }

    #[test]
    fn zustand_vergibt_ids_nach_dem_gespeicherten_verlauf() {
        let dir = std::env::temp_dir().join(format!("qb-zustand-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let pfad = dir.join("benachrichtigungen.json");
        let mut v = Verlauf::default();
        v.merke(&meldung(41, Dringlichkeit::Normal, -1));
        v.speichern(&pfad).expect("speichern");
        let (mut z, log) = Zustand::neu(Orte {
            verlauf: Some(pfad.clone()),
            ..Orte::default()
        });
        assert!(log.iter().any(|l| l.contains("1 Einträge")), "{log:?}");
        assert_eq!(z.neue_id(), 42);
        assert_eq!(z.verlauf.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ids_beginnen_bei_1_und_ueberspringen_0() {
        let (mut z, _) = Zustand::neu(Orte::default());
        assert_eq!(z.neue_id(), 1);
        assert_eq!(z.neue_id(), 2);
        z.naechste_id = u32::MAX;
        assert_eq!(z.neue_id(), u32::MAX);
        assert_eq!(z.neue_id(), 1);
    }

    #[test]
    fn nicht_stoeren_wird_gespeichert() {
        let dir = std::env::temp_dir().join(format!("qb-test-{}", std::process::id()));
        let datei = dir.join("nicht-stoeren");
        let _ = std::fs::remove_dir_all(&dir);
        let orte = || Orte {
            nicht_stoeren: Some(datei.clone()),
            ..Orte::default()
        };
        let (mut z, _) = Zustand::neu(orte());
        assert!(!z.nicht_stoeren);
        assert!(z.setze_nicht_stoeren(true));
        assert!(datei.exists());
        assert!(!z.setze_nicht_stoeren(true), "keine Änderung");
        let (z2, _) = Zustand::neu(orte());
        assert!(z2.nicht_stoeren, "nach Neustart noch an");
        assert!(z.setze_nicht_stoeren(false));
        assert!(!datei.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
