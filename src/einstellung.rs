//! Einstellungen aus `$XDG_CONFIG_HOME/querencia/benachrichtigungen.ini`
//! (0.2.0). Ohne GTK und in Unit-Tests prüfbar. Fehlt die Datei, gelten die
//! Standardwerte (Einblendungen an wie bei GNOME 2):
//!
//! ```ini
//! [Einblendung]
//! anzeigen=ja          # ja|nein – nein: nur noch kritische Meldungen als Popup
//! dauer_s=5            # Standarddauer normaler Meldungen in s (0 = bis geschlossen)
//! position=oben-rechts # oben-rechts|oben-mitte|unten-rechts
//! hinweis=ja           # Hinweis „in der Zentrale“ im Popup
//!
//! [Zentrale]
//! verlauf=200          # Einträge im Verlauf (10 bis 1000)
//! ```
//!
//! Der Dienst liest die Datei beim Start und erneut, sobald sich ihre
//! Änderungszeit ändert (bei jeder neuen Meldung und beim Öffnen der
//! Zentrale geprüft). Unbekannte Schlüssel und ungültige Werte werden
//! gemeldet und durch den Standard ersetzt.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::verlauf::{MAX_KAPAZITAET, MIN_KAPAZITAET, STANDARD_KAPAZITAET};

/// Wo die Popups stehen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    ObenRechts,
    ObenMitte,
    UntenRechts,
}

impl Position {
    pub fn name(self) -> &'static str {
        match self {
            Self::ObenRechts => "oben-rechts",
            Self::ObenMitte => "oben-mitte",
            Self::UntenRechts => "unten-rechts",
        }
    }

    fn aus_text(s: &str) -> Option<Self> {
        match s {
            "oben-rechts" => Some(Self::ObenRechts),
            "oben-mitte" => Some(Self::ObenMitte),
            "unten-rechts" => Some(Self::UntenRechts),
            _ => None,
        }
    }
}

pub const STANDARD_DAUER_S: u32 = 5;
pub const MAX_DAUER_S: u32 = 120;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Einstellungen {
    pub anzeigen: bool,
    pub dauer_s: u32,
    pub position: Position,
    pub hinweis: bool,
    pub verlauf: usize,
}

impl Default for Einstellungen {
    fn default() -> Self {
        Self {
            anzeigen: true,
            dauer_s: STANDARD_DAUER_S,
            position: Position::ObenRechts,
            hinweis: true,
            verlauf: STANDARD_KAPAZITAET,
        }
    }
}

fn ja_nein(s: &str) -> Option<bool> {
    match s.to_lowercase().as_str() {
        "ja" | "an" | "true" | "1" | "yes" | "on" => Some(true),
        "nein" | "aus" | "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

impl Einstellungen {
    /// Liest den Inhalt einer ini-Datei. Liefert die Einstellungen und
    /// Warnungen zu allem, was nicht verstanden wurde.
    pub fn aus_text(text: &str) -> (Self, Vec<String>) {
        let mut e = Self::default();
        let mut warnungen = Vec::new();
        let mut abschnitt = String::new();
        for (nr, zeile) in text.lines().enumerate() {
            let nr = nr + 1;
            let zeile = zeile.trim();
            if zeile.is_empty() || zeile.starts_with('#') || zeile.starts_with(';') {
                continue;
            }
            if let Some(name) = zeile.strip_prefix('[').and_then(|z| z.strip_suffix(']')) {
                abschnitt = name.trim().to_string();
                if abschnitt != "Einblendung" && abschnitt != "Zentrale" {
                    warnungen.push(format!("Zeile {nr}: unbekannter Abschnitt [{abschnitt}]"));
                }
                continue;
            }
            let Some((schluessel, wert)) = zeile.split_once('=') else {
                warnungen.push(format!("Zeile {nr}: ohne „=“ ignoriert"));
                continue;
            };
            let schluessel = schluessel.trim();
            // Kommentar hinter dem Wert erlauben („dauer_s=5  # Sekunden“).
            let wert = wert.split(" #").next().unwrap_or("").trim();
            let mut falsch = || {
                warnungen.push(format!(
                    "Zeile {nr}: {schluessel}={wert} ungültig, Standard gilt"
                ))
            };
            match (abschnitt.as_str(), schluessel) {
                ("Einblendung", "anzeigen") => match ja_nein(wert) {
                    Some(b) => e.anzeigen = b,
                    None => falsch(),
                },
                ("Einblendung", "hinweis") => match ja_nein(wert) {
                    Some(b) => e.hinweis = b,
                    None => falsch(),
                },
                ("Einblendung", "dauer_s") => match wert.parse::<u32>() {
                    Ok(n) if n <= MAX_DAUER_S => e.dauer_s = n,
                    _ => falsch(),
                },
                ("Einblendung", "position") => match Position::aus_text(wert) {
                    Some(p) => e.position = p,
                    None => falsch(),
                },
                ("Zentrale", "verlauf") => match wert.parse::<usize>() {
                    Ok(n) if (MIN_KAPAZITAET..=MAX_KAPAZITAET).contains(&n) => e.verlauf = n,
                    _ => falsch(),
                },
                (a, s) if a == "Einblendung" || a == "Zentrale" => {
                    warnungen.push(format!("Zeile {nr}: unbekannter Schlüssel {s} in [{a}]"));
                }
                _ => {}
            }
        }
        (e, warnungen)
    }
}

/// `$XDG_CONFIG_HOME/querencia/benachrichtigungen.ini`, sonst
/// `~/.config/querencia/benachrichtigungen.ini`.
pub fn ini_pfad(xdg_config_home: Option<&Path>, home: &Path) -> PathBuf {
    let basis = match xdg_config_home {
        Some(p) if p.is_absolute() => p.to_path_buf(),
        _ => home.join(".config"),
    };
    basis.join("querencia").join("benachrichtigungen.ini")
}

/// Hält die Einstellungen und liest die Datei neu, wenn sie sich ändert.
#[derive(Debug)]
pub struct Quelle {
    pfad: Option<PathBuf>,
    stand: Option<SystemTime>,
    vorhanden: bool,
    pub werte: Einstellungen,
}

impl Quelle {
    pub fn neu(pfad: Option<PathBuf>) -> Self {
        let mut q = Self {
            pfad,
            stand: None,
            vorhanden: false,
            werte: Einstellungen::default(),
        };
        q.laden();
        q
    }

    /// Liest neu, falls die Datei seit dem letzten Lesen erschienen,
    /// verschwunden oder geändert ist. Liefert Meldungen fürs Log (leer,
    /// wenn nichts neu gelesen wurde).
    pub fn auffrischen(&mut self) -> Vec<String> {
        let Some(pfad) = &self.pfad else {
            return Vec::new();
        };
        let meta = std::fs::metadata(pfad).ok();
        let stand = meta.as_ref().and_then(|m| m.modified().ok());
        if meta.is_some() == self.vorhanden && stand == self.stand {
            return Vec::new();
        }
        self.laden()
    }

    fn laden(&mut self) -> Vec<String> {
        let Some(pfad) = &self.pfad else {
            return Vec::new();
        };
        let mut log = Vec::new();
        match std::fs::read_to_string(pfad) {
            Ok(text) => {
                let (werte, warnungen) = Einstellungen::aus_text(&text);
                log.extend(
                    warnungen
                        .into_iter()
                        .map(|w| format!("{}: {w}", pfad.display())),
                );
                log.push(format!(
                    "Einstellungen aus {}: anzeigen={} dauer_s={} position={} hinweis={} verlauf={}",
                    pfad.display(),
                    if werte.anzeigen { "ja" } else { "nein" },
                    werte.dauer_s,
                    werte.position.name(),
                    if werte.hinweis { "ja" } else { "nein" },
                    werte.verlauf
                ));
                self.werte = werte;
                self.vorhanden = true;
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log.push(format!(
                        "{} nicht lesbar ({e}), Standard gilt",
                        pfad.display()
                    ));
                } else if self.vorhanden {
                    log.push(format!("{} entfernt, Standard gilt", pfad.display()));
                }
                self.werte = Einstellungen::default();
                self.vorhanden = false;
            }
        }
        self.stand = std::fs::metadata(pfad).and_then(|m| m.modified()).ok();
        log
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_ohne_datei() {
        let (e, w) = Einstellungen::aus_text("");
        assert_eq!(e, Einstellungen::default());
        assert!(w.is_empty());
        assert!(e.anzeigen && e.hinweis);
        assert_eq!(
            (e.dauer_s, e.position, e.verlauf),
            (5, Position::ObenRechts, 200)
        );
    }

    #[test]
    fn alle_schluessel() {
        let (e, w) = Einstellungen::aus_text(
            "# Kommentar\n[Einblendung]\nanzeigen = nein\ndauer_s=8  # Sekunden\n\
             position=unten-rechts\nhinweis=aus\n\n[Zentrale]\nverlauf=500\n",
        );
        assert!(w.is_empty(), "{w:?}");
        assert!(!e.anzeigen && !e.hinweis);
        assert_eq!(
            (e.dauer_s, e.position, e.verlauf),
            (8, Position::UntenRechts, 500)
        );
        let (e, _) = Einstellungen::aus_text("[Einblendung]\nposition=oben-mitte\ndauer_s=0");
        assert_eq!((e.position, e.dauer_s), (Position::ObenMitte, 0));
    }

    #[test]
    fn ungueltiges_meldet_und_nimmt_den_standard() {
        let (e, w) = Einstellungen::aus_text(
            "[Einblendung]\nanzeigen=vielleicht\ndauer_s=-3\nposition=links\nfarbe=rot\n\
             [Zentrale]\nverlauf=5\n[Sonst]\nx=1\nohne gleich\n",
        );
        assert_eq!(e, Einstellungen::default());
        assert_eq!(w.len(), 7, "{w:?}");
        assert!(w.iter().any(|z| z.contains("unbekannter Schlüssel farbe")));
        assert!(w.iter().any(|z| z.contains("[Sonst]")));
        let (e, w) =
            Einstellungen::aus_text("[Zentrale]\nverlauf=1001\n[Einblendung]\ndauer_s=121");
        assert_eq!((e.verlauf, e.dauer_s), (200, 5));
        assert_eq!(w.len(), 2);
    }

    #[test]
    fn schluessel_ausserhalb_der_abschnitte_zaehlen_nicht() {
        let (e, _) = Einstellungen::aus_text("anzeigen=nein\n[Zentrale]\nanzeigen=nein");
        assert!(e.anzeigen);
    }

    #[test]
    fn pfad_nach_xdg() {
        let home = Path::new("/home/x");
        assert_eq!(
            ini_pfad(None, home),
            PathBuf::from("/home/x/.config/querencia/benachrichtigungen.ini")
        );
        assert_eq!(
            ini_pfad(Some(Path::new("/p/config")), home),
            PathBuf::from("/p/config/querencia/benachrichtigungen.ini")
        );
        assert_eq!(
            ini_pfad(Some(Path::new("relativ")), home),
            PathBuf::from("/home/x/.config/querencia/benachrichtigungen.ini")
        );
    }

    #[test]
    fn quelle_liest_bei_aenderung_neu() {
        let d = std::env::temp_dir().join(format!("qb-ini-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("Ordner");
        let pfad = d.join("benachrichtigungen.ini");
        let mut q = Quelle::neu(Some(pfad.clone()));
        assert!(q.werte.anzeigen);
        assert!(q.auffrischen().is_empty(), "nichts geändert");
        std::fs::write(&pfad, "[Einblendung]\nanzeigen=nein\n").expect("ini");
        assert!(!q.auffrischen().is_empty());
        assert!(!q.werte.anzeigen);
        // gleiche Änderungszeit: kein erneutes Lesen
        assert!(q.auffrischen().is_empty());
        // Änderungszeit sicher anders setzen
        std::fs::write(&pfad, "[Einblendung]\nanzeigen=ja\nposition=oben-mitte\n").expect("ini");
        let f = std::fs::File::options()
            .write(true)
            .open(&pfad)
            .expect("öffnen");
        f.set_modified(SystemTime::now() + std::time::Duration::from_secs(5))
            .expect("mtime");
        q.auffrischen();
        assert_eq!(
            (q.werte.anzeigen, q.werte.position),
            (true, Position::ObenMitte)
        );
        std::fs::remove_file(&pfad).expect("weg");
        assert!(q.auffrischen().iter().any(|z| z.contains("entfernt")));
        assert_eq!(q.werte, Einstellungen::default());
        let _ = std::fs::remove_dir_all(&d);
    }
}
