//! Datenmodell des Dienstes: Meldung mit Hints, Dringlichkeit, Anzeigedauer,
//! Verlauf (letzte 50 Meldungen) und der von D-Bus- und GTK-Seite gemeinsam
//! benutzte Zustand. Alles hier läuft ohne Display und ist in Unit-Tests
//! prüfbar.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::PathBuf;
use std::time::Duration;

use zbus::zvariant::{OwnedValue, Value};

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
        if self.bits_je_kanal != 8 || self.breite <= 0 || self.hoehe <= 0 || self.breite > 4096 || self.hoehe > 4096 {
            return false;
        }
        let kanaele = if self.alpha { 4 } else { 3 };
        if self.kanaele != kanaele || self.zeilenlaenge < self.breite * kanaele {
            return false;
        }
        let noetig = (self.hoehe as usize - 1) * self.zeilenlaenge as usize + (self.breite * kanaele) as usize;
        self.daten.len() >= noetig
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
    pub fn anzeigedauer(&self) -> Option<Duration> {
        anzeigedauer(self.dringlichkeit, self.expire_timeout)
    }
}

/// Anzeigedauer nach Dringlichkeit und `expire_timeout` (Millisekunden):
/// kritische Meldungen bleiben immer stehen; 0 heißt „nie ablaufen“; ein
/// positiver Wert gilt; -1 (oder jeder andere negative Wert) nimmt den
/// Standard: 3 s bei niedriger, 5 s bei normaler Dringlichkeit.
pub fn anzeigedauer(dringlichkeit: Dringlichkeit, expire_timeout: i32) -> Option<Duration> {
    match dringlichkeit {
        Dringlichkeit::Kritisch => None,
        _ if expire_timeout == 0 => None,
        _ if expire_timeout > 0 => Some(Duration::from_millis(expire_timeout as u64)),
        Dringlichkeit::Niedrig => Some(Duration::from_secs(3)),
        Dringlichkeit::Normal => Some(Duration::from_secs(5)),
    }
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
    namen.iter().find_map(|n| match hints.get(*n).map(|v| &**v) {
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
    let Value::Bool(alpha) = &f[3] else { return None };
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

/// Ein Eintrag des Verlaufs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eintrag {
    pub id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub dringlichkeit: Dringlichkeit,
    pub zeit: i64,
}

impl Eintrag {
    pub fn aus(m: &Meldung) -> Self {
        Self {
            id: m.id,
            app_name: m.app_name.clone(),
            app_icon: m.app_icon.clone(),
            summary: m.summary.clone(),
            body: m.body.clone(),
            dringlichkeit: m.dringlichkeit,
            zeit: m.zeit,
        }
    }

    /// Form für D-Bus: (u id, s app, s symbol, s betreff, s text, y dringlichkeit, x zeit).
    pub fn als_tupel(&self) -> (u32, String, String, String, String, u8, i64) {
        (
            self.id,
            self.app_name.clone(),
            self.app_icon.clone(),
            self.summary.clone(),
            self.body.clone(),
            self.dringlichkeit.als_byte(),
            self.zeit,
        )
    }
}

/// Die letzten `KAPAZITAET` Meldungen, älteste zuerst. Ein Ersatz
/// (`replaces_id`) aktualisiert den vorhandenen Eintrag an seiner Stelle.
#[derive(Debug, Default)]
pub struct Verlauf {
    eintraege: VecDeque<Eintrag>,
}

pub const KAPAZITAET: usize = 50;

impl Verlauf {
    pub fn merke(&mut self, m: &Meldung) {
        if m.transient {
            return;
        }
        let neu = Eintrag::aus(m);
        if let Some(alt) = self.eintraege.iter_mut().find(|e| e.id == m.id) {
            *alt = neu;
            return;
        }
        if self.eintraege.len() == KAPAZITAET {
            self.eintraege.pop_front();
        }
        self.eintraege.push_back(neu);
    }

    pub fn eintraege(&self) -> impl Iterator<Item = &Eintrag> {
        self.eintraege.iter()
    }

    pub fn len(&self) -> usize {
        self.eintraege.len()
    }

    pub fn is_empty(&self) -> bool {
        self.eintraege.is_empty()
    }

    pub fn leeren(&mut self) {
        self.eintraege.clear();
    }
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
}

impl Zustand {
    pub fn neu(nicht_stoeren_datei: Option<PathBuf>) -> Self {
        let nicht_stoeren = nicht_stoeren_datei.as_ref().is_some_and(|p| p.exists());
        Self {
            naechste_id: 1,
            nicht_stoeren,
            verlauf: Verlauf::default(),
            aktiv: BTreeSet::new(),
            sichtbar: 0,
            wartend: 0,
            nicht_stoeren_datei,
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
                p.parent().map(std::fs::create_dir_all).transpose().and_then(|_| std::fs::write(p, b"an\n"))
            } else {
                match std::fs::remove_file(p) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                    _ => Ok(()),
                }
            };
            if let Err(e) = ergebnis {
                eprintln!("querencia-benachrichtigungen: Nicht-stören-Zustand nicht gespeichert ({}): {e}", p.display());
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

    fn meldung(id: u32, dringlichkeit: Dringlichkeit, timeout: i32) -> Meldung {
        let mut hints = HashMap::new();
        hints.insert("urgency".to_string(), OwnedValue::from(dringlichkeit.als_byte()));
        Meldung::neu(id, "app".into(), String::new(), format!("Betreff {id}"), "Text".into(), vec![], &hints, timeout, 1_000 + id as i64)
    }

    #[test]
    fn anzeigedauer_standardwerte() {
        assert_eq!(anzeigedauer(Dringlichkeit::Normal, -1), Some(Duration::from_secs(5)));
        assert_eq!(anzeigedauer(Dringlichkeit::Niedrig, -1), Some(Duration::from_secs(3)));
        assert_eq!(anzeigedauer(Dringlichkeit::Kritisch, -1), None);
    }

    #[test]
    fn anzeigedauer_explizit() {
        assert_eq!(anzeigedauer(Dringlichkeit::Normal, 1500), Some(Duration::from_millis(1500)));
        assert_eq!(anzeigedauer(Dringlichkeit::Niedrig, 10_000), Some(Duration::from_secs(10)));
        assert_eq!(anzeigedauer(Dringlichkeit::Normal, 0), None, "0 = nie ablaufen");
        assert_eq!(anzeigedauer(Dringlichkeit::Kritisch, 2000), None, "kritisch bleibt stehen");
        assert_eq!(anzeigedauer(Dringlichkeit::Normal, -7), Some(Duration::from_secs(5)), "negativ = Standard");
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
            (OwnedValue::from("kritisch"), Dringlichkeit::Normal),
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
        hints.insert("image-path".to_string(), OwnedValue::from("/tmp/x.png"));
        hints.insert("desktop-entry".to_string(), OwnedValue::from("org.querencia.Editor"));
        hints.insert("transient".to_string(), OwnedValue::from(true));
        hints.insert("category".to_string(), OwnedValue::from("email.arrived"));
        let m = Meldung::neu(1, "a".into(), "".into(), "s".into(), "b".into(), vec![], &hints, -1, 0);
        assert_eq!(m.bild_pfad.as_deref(), Some("/tmp/x.png"));
        assert_eq!(m.desktop_entry.as_deref(), Some("org.querencia.Editor"));
        assert!(m.transient);
        assert!(!m.resident);
        assert_eq!(m.category.as_deref(), Some("email.arrived"));
    }

    fn bildwert(b: i32, h: i32, zl: i32, alpha: bool, bps: i32, kan: i32, daten: Vec<u8>) -> OwnedValue {
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
        hints.insert("image-data".to_string(), bildwert(2, 2, 8, true, 8, 4, vec![7; 16]));
        let b = hint_bild(&hints).expect("Bild");
        assert_eq!((b.breite, b.hoehe, b.zeilenlaenge, b.alpha, b.kanaele), (2, 2, 8, true, 4));
        assert_eq!(b.daten.len(), 16);
        // zu kurzer Puffer
        hints.insert("image-data".to_string(), bildwert(2, 2, 8, true, 8, 4, vec![7; 10]));
        assert!(hint_bild(&hints).is_none());
        // 16 Bit je Kanal kann GTK so nicht
        hints.insert("image-data".to_string(), bildwert(2, 2, 8, false, 16, 3, vec![7; 32]));
        assert!(hint_bild(&hints).is_none());
        // alter Name
        hints.clear();
        hints.insert("icon_data".to_string(), bildwert(1, 1, 3, false, 8, 3, vec![1, 2, 3]));
        assert!(hint_bild(&hints).is_some());
    }

    #[test]
    fn aktionen_paare_und_default() {
        let m = Meldung::neu(
            1, "a".into(), "".into(), "s".into(), "b".into(),
            vec!["default".into(), "Öffnen".into(), "ja".into(), "Ja".into(), "einsam".into()],
            &HashMap::new(), -1, 0,
        );
        assert_eq!(m.actions.len(), 2);
        assert!(m.hat_default_aktion());
        let knoepfe: Vec<_> = m.knopf_aktionen().collect();
        assert_eq!(knoepfe, vec![&("ja".to_string(), "Ja".to_string())]);
    }

    #[test]
    fn verlauf_haelt_hoechstens_50() {
        let mut v = Verlauf::default();
        for i in 1..=60 {
            v.merke(&meldung(i, Dringlichkeit::Normal, -1));
        }
        assert_eq!(v.len(), KAPAZITAET);
        let ids: Vec<u32> = v.eintraege().map(|e| e.id).collect();
        assert_eq!(ids.first(), Some(&11));
        assert_eq!(ids.last(), Some(&60));
    }

    #[test]
    fn verlauf_ersatz_aktualisiert_an_ort_und_stelle() {
        let mut v = Verlauf::default();
        v.merke(&meldung(1, Dringlichkeit::Normal, -1));
        v.merke(&meldung(2, Dringlichkeit::Normal, -1));
        let mut ersatz = meldung(1, Dringlichkeit::Kritisch, -1);
        ersatz.summary = "Neu".into();
        v.merke(&ersatz);
        let e: Vec<_> = v.eintraege().collect();
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].id, 1);
        assert_eq!(e[0].summary, "Neu");
        assert_eq!(e[0].dringlichkeit, Dringlichkeit::Kritisch);
    }

    #[test]
    fn verlauf_ohne_transiente() {
        let mut v = Verlauf::default();
        let mut m = meldung(1, Dringlichkeit::Normal, -1);
        m.transient = true;
        v.merke(&m);
        assert!(v.is_empty());
    }

    #[test]
    fn eintrag_tupel() {
        let m = meldung(5, Dringlichkeit::Niedrig, -1);
        let t = Eintrag::aus(&m).als_tupel();
        assert_eq!(t.0, 5);
        assert_eq!(t.3, "Betreff 5");
        assert_eq!(t.5, 0);
        assert_eq!(t.6, 1005);
    }

    #[test]
    fn ids_beginnen_bei_1_und_ueberspringen_0() {
        let mut z = Zustand::neu(None);
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
        let mut z = Zustand::neu(Some(datei.clone()));
        assert!(!z.nicht_stoeren);
        assert!(z.setze_nicht_stoeren(true));
        assert!(datei.exists());
        assert!(!z.setze_nicht_stoeren(true), "keine Änderung");
        let z2 = Zustand::neu(Some(datei.clone()));
        assert!(z2.nicht_stoeren, "nach Neustart noch an");
        assert!(z.setze_nicht_stoeren(false));
        assert!(!datei.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
