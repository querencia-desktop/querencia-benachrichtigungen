//! Verlauf der Mitteilungszentrale (0.2.0): Ringpuffer der letzten Meldungen
//! mit Zustand gelesen/ungelesen, gespeichert als JSON in
//! `$XDG_STATE_HOME/querencia/benachrichtigungen.json` (Rechte 0600). Ohne
//! GTK und in Unit-Tests prüfbar.
//!
//! Gespeichert werden Zeit, Programm, Symbolangaben, Betreff, Text,
//! Dringlichkeit, Kategorie und Aktionen. Bilddaten aus `image-data` bleiben
//! nur im Speicher (auf 48 px verkleinert) und gehen beim Neustart verloren.
//! Meldungen mit dem Hint `transient` kommen nicht in den Verlauf.

use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::modell::{BildDaten, Dringlichkeit, Meldung};

/// Standardgröße des Ringpuffers (`[Zentrale] verlauf=` in der ini).
pub const STANDARD_KAPAZITAET: usize = 200;
/// Grenzen für `verlauf=`.
pub const MIN_KAPAZITAET: usize = 10;
pub const MAX_KAPAZITAET: usize = 1000;
/// So viele Einträge liefert die alte Methode `Verlauf` (0.1.x) und `--verlauf`.
pub const ALT_ANZAHL: usize = 50;
/// Kantenlänge, auf die `image-data` für den Verlauf verkleinert wird.
pub const BILD_KANTE: i32 = 48;
const FORMAT: u32 = 1;

/// Ein Eintrag des Verlaufs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Eintrag {
    pub id: u32,
    /// Unixzeit des Eintreffens (bei Ersatz: der letzten Fassung).
    pub zeit: i64,
    pub app_name: String,
    #[serde(default)]
    pub desktop_entry: String,
    #[serde(default)]
    pub app_icon: String,
    #[serde(default)]
    pub bild_pfad: String,
    pub summary: String,
    pub body: String,
    /// 0 niedrig, 1 normal, 2 kritisch.
    pub dringlichkeit: u8,
    #[serde(default)]
    pub kategorie: String,
    /// Paare (Schlüssel, Beschriftung), `default` eingeschlossen.
    #[serde(default)]
    pub aktionen: Vec<(String, String)>,
    #[serde(default)]
    pub gelesen: bool,
    /// Aus dem laufenden Dienst (nicht aus der Datei): nur dann kann der
    /// Absender eine Aktion noch empfangen.
    #[serde(skip)]
    pub aus_diesem_lauf: bool,
    #[serde(skip)]
    pub bild_daten: Option<BildDaten>,
}

/// Form eines Eintrags für D-Bus (Signatur `(uxsssssysa(ss)b)`):
/// id, zeit, app_name, desktop_entry, symbol, summary, body, dringlichkeit,
/// kategorie, aktionen, gelesen.
pub type EintragTupel = (
    u32,
    i64,
    String,
    String,
    String,
    String,
    String,
    u8,
    String,
    Vec<(String, String)>,
    bool,
);

impl Eintrag {
    pub fn aus(m: &Meldung) -> Self {
        Self {
            id: m.id,
            zeit: m.zeit,
            app_name: m.app_name.clone(),
            desktop_entry: m.desktop_entry.clone().unwrap_or_default(),
            app_icon: m.app_icon.clone(),
            bild_pfad: m.bild_pfad.clone().unwrap_or_default(),
            summary: m.summary.clone(),
            body: m.body.clone(),
            dringlichkeit: m.dringlichkeit.als_byte(),
            kategorie: m.category.clone().unwrap_or_default(),
            aktionen: m.actions.clone(),
            gelesen: false,
            aus_diesem_lauf: true,
            bild_daten: m
                .bild_daten
                .as_ref()
                .and_then(|b| b.verkleinert(BILD_KANTE)),
        }
    }

    pub fn dringlichkeit(&self) -> Dringlichkeit {
        Dringlichkeit::aus_byte(self.dringlichkeit)
    }

    /// Symbolangabe für Dritte: `image-path` vor `app_icon` (Pfad, URI oder
    /// Themenname); leer, wenn es keine gibt (dann gilt `desktop_entry`).
    pub fn symbol(&self) -> &str {
        if !self.bild_pfad.is_empty() {
            &self.bild_pfad
        } else {
            &self.app_icon
        }
    }

    /// Gruppe in der Zentrale: Programmname, sonst `desktop-entry`.
    pub fn gruppe(&self) -> &str {
        if !self.app_name.trim().is_empty() {
            self.app_name.trim()
        } else {
            self.desktop_entry.trim()
        }
    }

    /// Ob `suche` (klein geschrieben) in Programm, Betreff oder Text vorkommt.
    pub fn passt(&self, suche: &str) -> bool {
        let suche = suche.trim().to_lowercase();
        suche.is_empty()
            || [
                &self.app_name,
                &self.summary,
                &self.body,
                &self.desktop_entry,
            ]
            .iter()
            .any(|f| f.to_lowercase().contains(&suche))
    }

    pub fn als_tupel(&self) -> EintragTupel {
        (
            self.id,
            self.zeit,
            self.app_name.clone(),
            self.desktop_entry.clone(),
            self.symbol().to_string(),
            self.summary.clone(),
            self.body.clone(),
            self.dringlichkeit,
            self.kategorie.clone(),
            self.aktionen.clone(),
            self.gelesen,
        )
    }

    /// Form der alten Methode `Verlauf` (0.1.x): (u, s, s, s, s, y, x).
    pub fn als_alt_tupel(&self) -> (u32, String, String, String, String, u8, i64) {
        (
            self.id,
            self.app_name.clone(),
            self.app_icon.clone(),
            self.summary.clone(),
            self.body.clone(),
            self.dringlichkeit,
            self.zeit,
        )
    }
}

/// Ergebnis von [`Verlauf::merke`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gemerkt {
    Neu,
    Ersetzt,
    /// `transient`: nicht gespeichert.
    Nein,
}

/// Ringpuffer, älteste zuerst. Ein Ersatz (`replaces_id`) aktualisiert den
/// vorhandenen Eintrag an seiner Stelle und macht ihn wieder ungelesen.
#[derive(Debug)]
pub struct Verlauf {
    eintraege: VecDeque<Eintrag>,
    kapazitaet: usize,
}

impl Default for Verlauf {
    fn default() -> Self {
        Self::neu(STANDARD_KAPAZITAET)
    }
}

#[derive(Serialize, Deserialize)]
struct Datei {
    format: u32,
    eintraege: Vec<Eintrag>,
}

impl Verlauf {
    pub fn neu(kapazitaet: usize) -> Self {
        Self {
            eintraege: VecDeque::new(),
            kapazitaet: kapazitaet.clamp(MIN_KAPAZITAET, MAX_KAPAZITAET),
        }
    }

    pub fn kapazitaet(&self) -> usize {
        self.kapazitaet
    }

    /// Neue Kapazität; überzählige älteste Einträge fallen weg.
    pub fn setze_kapazitaet(&mut self, n: usize) -> bool {
        let n = n.clamp(MIN_KAPAZITAET, MAX_KAPAZITAET);
        let alt_len = self.eintraege.len();
        self.kapazitaet = n;
        while self.eintraege.len() > n {
            self.eintraege.pop_front();
        }
        alt_len != self.eintraege.len()
    }

    pub fn merke(&mut self, m: &Meldung) -> Gemerkt {
        if m.transient {
            return Gemerkt::Nein;
        }
        let neu = Eintrag::aus(m);
        if let Some(alt) = self.eintraege.iter_mut().find(|e| e.id == m.id) {
            *alt = neu;
            return Gemerkt::Ersetzt;
        }
        while self.eintraege.len() >= self.kapazitaet {
            self.eintraege.pop_front();
        }
        self.eintraege.push_back(neu);
        Gemerkt::Neu
    }

    pub fn eintraege(&self) -> impl DoubleEndedIterator<Item = &Eintrag> {
        self.eintraege.iter()
    }

    pub fn eintrag(&self, id: u32) -> Option<&Eintrag> {
        self.eintraege.iter().find(|e| e.id == id)
    }

    /// Die letzten `n` Einträge, älteste zuerst (alte Methode `Verlauf`).
    pub fn letzte(&self, n: usize) -> impl Iterator<Item = &Eintrag> {
        self.eintraege
            .iter()
            .skip(self.eintraege.len().saturating_sub(n))
    }

    pub fn len(&self) -> usize {
        self.eintraege.len()
    }

    pub fn is_empty(&self) -> bool {
        self.eintraege.is_empty()
    }

    pub fn ungelesen(&self) -> usize {
        self.eintraege.iter().filter(|e| !e.gelesen).count()
    }

    pub fn hoechste_id(&self) -> u32 {
        self.eintraege.iter().map(|e| e.id).max().unwrap_or(0)
    }

    /// Markiert `ids` (leer: alle) als gelesen; liefert die geänderten IDs.
    pub fn als_gelesen(&mut self, ids: &[u32]) -> Vec<u32> {
        let mut geaendert = Vec::new();
        for e in self.eintraege.iter_mut() {
            if !e.gelesen && (ids.is_empty() || ids.contains(&e.id)) {
                e.gelesen = true;
                geaendert.push(e.id);
            }
        }
        geaendert
    }

    /// Entfernt `ids`; liefert die tatsächlich entfernten.
    pub fn entfernen(&mut self, ids: &[u32]) -> Vec<u32> {
        let mut weg = Vec::new();
        self.eintraege.retain(|e| {
            let treffer = ids.contains(&e.id);
            if treffer {
                weg.push(e.id);
            }
            !treffer
        });
        weg
    }

    /// Leert den Verlauf; liefert die Zahl der entfernten Einträge.
    pub fn leeren(&mut self) -> usize {
        let n = self.eintraege.len();
        self.eintraege.clear();
        n
    }

    pub fn als_json(&self) -> String {
        let datei = Datei {
            format: FORMAT,
            eintraege: self.eintraege.iter().cloned().collect(),
        };
        serde_json::to_string_pretty(&datei).unwrap_or_else(|_| "{}".into())
    }

    /// Liest den Verlauf aus JSON; die Kapazität gilt (älteste fallen weg).
    pub fn aus_json(text: &str, kapazitaet: usize) -> Result<Self, String> {
        let datei: Datei = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if datei.format != FORMAT {
            return Err(format!("unbekanntes Format {}", datei.format));
        }
        let mut v = Self::neu(kapazitaet);
        for e in datei.eintraege {
            // Doppelte IDs (von Hand bearbeitete Datei): der spätere gewinnt.
            v.eintraege.retain(|alt| alt.id != e.id);
            v.eintraege.push_back(e);
        }
        v.setze_kapazitaet(v.kapazitaet);
        Ok(v)
    }

    /// Schreibt den Verlauf atomar (Hilfsdatei, dann Umbenennen) mit Rechten
    /// 0600. Der Ordner wird bei Bedarf angelegt.
    pub fn speichern(&self, pfad: &Path) -> std::io::Result<()> {
        use std::os::unix::fs::OpenOptionsExt;
        if let Some(ordner) = pfad.parent() {
            std::fs::create_dir_all(ordner)?;
        }
        let tmp = hilfsname(pfad);
        let ergebnis = (|| {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)?;
            f.write_all(self.als_json().as_bytes())?;
            f.write_all(b"\n")?;
            f.sync_all()?;
            std::fs::rename(&tmp, pfad)
        })();
        if ergebnis.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        ergebnis
    }

    /// Lädt den Verlauf. Fehlt die Datei, ist er leer. Ist sie unlesbar,
    /// wird sie als `<name>.defekt` beiseitegelegt; der Verlauf beginnt leer
    /// und der Text erklärt, was geschah.
    pub fn laden(pfad: &Path, kapazitaet: usize) -> (Self, Option<String>) {
        let text = match std::fs::read_to_string(pfad) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return (Self::neu(kapazitaet), None);
            }
            Err(e) => {
                return (
                    Self::neu(kapazitaet),
                    Some(format!("Verlauf {} nicht lesbar: {e}", pfad.display())),
                );
            }
        };
        match Self::aus_json(&text, kapazitaet) {
            Ok(v) => (v, None),
            Err(e) => {
                let defekt = pfad.with_extension("json.defekt");
                let _ = std::fs::rename(pfad, &defekt);
                (
                    Self::neu(kapazitaet),
                    Some(format!(
                        "Verlauf {} unbrauchbar ({e}), beiseitegelegt als {}",
                        pfad.display(),
                        defekt.display()
                    )),
                )
            }
        }
    }
}

fn hilfsname(pfad: &Path) -> PathBuf {
    let name = pfad
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "verlauf".into());
    pfad.with_file_name(format!(".{name}.tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn meldung(id: u32, summary: &str) -> Meldung {
        Meldung::neu(
            id,
            "Probe".into(),
            "mail-unread".into(),
            summary.into(),
            "Text <b>fett</b>".into(),
            vec!["default".into(), "Öffnen".into(), "ja".into(), "Ja".into()],
            &HashMap::new(),
            -1,
            1_000 + id as i64,
        )
    }

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("qb-verlauf-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn ringpuffer_haelt_die_kapazitaet() {
        let mut v = Verlauf::neu(200);
        for i in 1..=230 {
            assert_eq!(v.merke(&meldung(i, "x")), Gemerkt::Neu);
        }
        assert_eq!(v.len(), 200);
        assert_eq!(v.eintraege().next().map(|e| e.id), Some(31));
        assert_eq!(v.eintraege().last().map(|e| e.id), Some(230));
        assert_eq!(v.letzte(ALT_ANZAHL).count(), 50);
        assert_eq!(v.letzte(ALT_ANZAHL).next().map(|e| e.id), Some(181));
    }

    #[test]
    fn kapazitaet_wird_begrenzt_und_kuerzt() {
        assert_eq!(Verlauf::neu(1).kapazitaet(), MIN_KAPAZITAET);
        assert_eq!(Verlauf::neu(1_000_000).kapazitaet(), MAX_KAPAZITAET);
        let mut v = Verlauf::neu(100);
        for i in 1..=40 {
            v.merke(&meldung(i, "x"));
        }
        assert!(v.setze_kapazitaet(20));
        assert_eq!(v.len(), 20);
        assert_eq!(v.eintraege().next().map(|e| e.id), Some(21));
        assert!(!v.setze_kapazitaet(500));
    }

    #[test]
    fn transiente_bleiben_draussen() {
        let mut v = Verlauf::default();
        let mut m = meldung(1, "x");
        m.transient = true;
        assert_eq!(v.merke(&m), Gemerkt::Nein);
        assert!(v.is_empty());
    }

    #[test]
    fn ersatz_aktualisiert_und_macht_ungelesen() {
        let mut v = Verlauf::default();
        v.merke(&meldung(1, "alt"));
        v.merke(&meldung(2, "zwei"));
        assert_eq!(v.als_gelesen(&[1]), vec![1]);
        assert_eq!(v.ungelesen(), 1);
        assert_eq!(v.merke(&meldung(1, "neu")), Gemerkt::Ersetzt);
        assert_eq!(v.len(), 2);
        let e = v.eintrag(1).expect("Eintrag 1");
        assert_eq!(e.summary, "neu");
        assert!(!e.gelesen);
        assert_eq!(
            v.eintraege().next().map(|e| e.id),
            Some(1),
            "an Ort und Stelle"
        );
    }

    #[test]
    fn gelesen_entfernen_leeren() {
        let mut v = Verlauf::default();
        for i in 1..=5 {
            v.merke(&meldung(i, "x"));
        }
        assert_eq!(v.ungelesen(), 5);
        assert_eq!(v.als_gelesen(&[2, 4, 99]), vec![2, 4]);
        assert_eq!(v.als_gelesen(&[2]), Vec::<u32>::new(), "schon gelesen");
        assert_eq!(v.ungelesen(), 3);
        assert_eq!(v.entfernen(&[1, 99]), vec![1]);
        assert_eq!(v.len(), 4);
        assert_eq!(v.als_gelesen(&[]), vec![3, 5], "leer heißt alle");
        assert_eq!(v.ungelesen(), 0);
        assert_eq!(v.leeren(), 4);
        assert!(v.is_empty());
    }

    #[test]
    fn eintrag_felder_tupel_und_suche() {
        let mut v = Verlauf::default();
        let mut m = meldung(7, "Neue Mail");
        m.bild_pfad = Some("/tmp/bild.png".into());
        m.desktop_entry = Some("org.test.Probe".into());
        m.category = Some("email.arrived".into());
        v.merke(&m);
        let e = v.eintrag(7).expect("Eintrag");
        assert_eq!(e.symbol(), "/tmp/bild.png");
        assert_eq!(e.gruppe(), "Probe");
        assert!(e.passt("mail") && e.passt("FETT") && e.passt("probe") && e.passt(" "));
        assert!(!e.passt("kalender"));
        let t = e.als_tupel();
        assert_eq!((t.0, t.1, t.7), (7, 1007, 1));
        assert_eq!(t.3, "org.test.Probe");
        assert_eq!(t.8, "email.arrived");
        assert_eq!(t.9.len(), 2);
        assert!(!t.10);
        let a = e.als_alt_tupel();
        assert_eq!((a.0, a.2.as_str(), a.6), (7, "mail-unread", 1007));
        let mut ohne = e.clone();
        ohne.app_name = " ".into();
        assert_eq!(ohne.gruppe(), "org.test.Probe");
    }

    #[test]
    fn json_hin_und_zurueck() {
        let mut v = Verlauf::default();
        for i in 1..=3 {
            v.merke(&meldung(i, &format!("Betreff {i} „Ümlaut“")));
        }
        v.als_gelesen(&[2]);
        let w = Verlauf::aus_json(&v.als_json(), 200).expect("JSON");
        assert_eq!(w.len(), 3);
        let a: Vec<_> = v
            .eintraege()
            .map(|e| (e.id, &e.summary, e.gelesen))
            .collect();
        let b: Vec<_> = w
            .eintraege()
            .map(|e| (e.id, &e.summary, e.gelesen))
            .collect();
        assert_eq!(a, b);
        assert!(w.eintraege().all(|e| !e.aus_diesem_lauf), "aus der Datei");
        assert_eq!(w.eintrag(1).map(|e| e.aktionen.len()), Some(2));
        assert_eq!(w.hoechste_id(), 3);
        // Kapazität beim Laden
        let klein = Verlauf::aus_json(&v.als_json(), 10).expect("JSON");
        assert_eq!(klein.len(), 3);
        assert!(Verlauf::aus_json("{\"format\":9,\"eintraege\":[]}", 200).is_err());
        assert!(Verlauf::aus_json("kein json", 200).is_err());
    }

    #[test]
    fn speichern_und_laden_mit_rechten() {
        use std::os::unix::fs::PermissionsExt;
        let d = tmpdir("datei");
        let pfad = d.join("querencia").join("benachrichtigungen.json");
        let (leer, hinweis) = Verlauf::laden(&pfad, 200);
        assert!(leer.is_empty() && hinweis.is_none(), "fehlende Datei: leer");
        let mut v = Verlauf::default();
        v.merke(&meldung(1, "eins"));
        v.speichern(&pfad).expect("speichern");
        let modus = std::fs::metadata(&pfad)
            .expect("Datei")
            .permissions()
            .mode();
        assert_eq!(modus & 0o777, 0o600);
        assert!(!d.join("querencia/.benachrichtigungen.json.tmp").exists());
        let (w, hinweis) = Verlauf::laden(&pfad, 200);
        assert!(hinweis.is_none());
        assert_eq!(w.eintrag(1).map(|e| e.summary.as_str()), Some("eins"));
        // defekte Datei wird beiseitegelegt
        std::fs::write(&pfad, "{ kaputt").expect("schreiben");
        let (w, hinweis) = Verlauf::laden(&pfad, 200);
        assert!(w.is_empty());
        assert!(hinweis.is_some_and(|h| h.contains("beiseitegelegt")));
        assert!(d.join("querencia/benachrichtigungen.json.defekt").exists());
        assert!(!pfad.exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
