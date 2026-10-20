//! Übersetzungen über GNU gettext (Textdomäne `querencia-benachrichtigungen`).
//!
//! Quellsprache im Code ist Englisch (msgid englisch, wie bei GNOME und
//! Debian); Deutsch kommt aus `po/de.po`. Ohne passenden Katalog zeigt der
//! Dienst die englischen Texte. Die Sprache wählt gettext wie üblich über
//! `LANGUAGE`, `LC_ALL`, `LC_MESSAGES` und `LANG` (die Sitzung setzt
//! `LANGUAGE=de` für den Autostart).
//!
//! Übersetzt ist nur, was der Dienst selbst zeigt (Popups, Mitteilungszentrale,
//! Starter); Betreff, Text und Aktionen kommen vom Absender. Befehlszeile,
//! `--hilfe`, Log und D-Bus-Fehlertexte bleiben deutsch.
//!
//! Die Kataloge (`<lang>/LC_MESSAGES/querencia-benachrichtigungen.mo`) sucht
//! der Dienst zuerst neben der Binary (`<ablage>/share/locale`, unveränderliche
//! Ablage unter `~/opt/querencia/benachrichtigungen/<sha16>/`), dann im Präfix
//! der Binary (`/usr/bin/../share/locale`), zuletzt in `/usr/share/locale`.
//! `QUERENCIA_BENACHRICHTIGUNGEN_LOCALEDIR` setzt das Verzeichnis ausdrücklich
//! (Tests). Muster und Werkzeug wie im Querencia Editor 0.2.0
//! (`werkzeug/i18n.py`).

use std::path::{Path, PathBuf};

pub use gettextrs::gettext;
use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};

/// Übersetzt `msgid` und setzt `args` der Reihe nach für die Platzhalter
/// `{name}` ein (benannte Platzhalter, damit Übersetzungen sie umstellen
/// können; `werkzeug/i18n.py pruefen` vergleicht sie).
pub fn gettext_f(msgid: &str, args: &[(&str, &str)]) -> String {
    let mut text = gettext(msgid);
    for (name, wert) in args {
        text = text.replace(&format!("{{{name}}}"), wert);
    }
    text
}

/// Textdomäne = Name der .mo-Dateien.
pub const DOMAIN: &str = "querencia-benachrichtigungen";

/// Textdomäne binden; vor dem Start von GTK aufrufen. Die Locale aus der
/// Umgebung setzt GTK selbst beim Start (`gtk_init` ruft `setlocale(LC_ALL,
/// "")`); alle Texte entstehen erst danach (Popups). Ein eigenes `setlocale`
/// wäre in gettext-rs 0.8 `unsafe` und ist nicht nötig.
pub fn init() {
    let dir = locale_dir();
    if let Err(err) = bindtextdomain(DOMAIN, dir.as_path()) {
        eprintln!(
            "querencia-benachrichtigungen: Übersetzungen nicht gebunden ({}): {err}",
            dir.display()
        );
    }
    let _ = bind_textdomain_codeset(DOMAIN, "UTF-8");
    let _ = textdomain(DOMAIN);
}

/// Verzeichnis der Kataloge (siehe Modulbeschreibung).
pub fn locale_dir() -> PathBuf {
    if let Some(dir) =
        std::env::var_os("QUERENCIA_BENACHRICHTIGUNGEN_LOCALEDIR").filter(|d| !d.is_empty())
    {
        return PathBuf::from(dir);
    }
    // current_exe() löst den Link ~/opt/querencia/bin/… auf die Ablage auf.
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.canonicalize().ok())
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    locale_dir_for(exe_dir.as_deref())
}

fn locale_dir_for(exe_dir: Option<&Path>) -> PathBuf {
    if let Some(exe_dir) = exe_dir {
        let beside = exe_dir.join("share/locale");
        if beside.is_dir() {
            return beside;
        }
        if let Some(prefix) = exe_dir.parent() {
            let prefixed = prefix.join("share/locale");
            if prefixed.is_dir() {
                return prefixed;
            }
        }
    }
    PathBuf::from("/usr/share/locale")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untranslated_text_falls_back_to_the_english_msgid() {
        // Ohne gebundenen Katalog liefert gettext die msgid. Die msgid steht
        // in einer Variablen, damit sie nicht in die .pot gerät.
        let msgid = "not a catalog entry";
        assert_eq!(gettext(msgid), "not a catalog entry");
    }

    /// Deutsch hängt an `po/de.po`: jede msgid der Vorlage muss dort
    /// übersetzt sein (sonst sähe die deutsche Sitzung Englisch).
    #[test]
    fn german_catalog_is_complete() {
        let pot = include_str!("../po/querencia-benachrichtigungen.pot");
        let de = include_str!("../po/de.po");
        assert!(!de.contains("#, fuzzy"), "de.po enthält fuzzy-Einträge");
        let ids = |text: &str| -> Vec<String> {
            text.split("\n\n")
                .filter_map(|block| {
                    let id: Vec<&str> = block
                        .lines()
                        .skip_while(|l| !l.starts_with("msgid "))
                        .take_while(|l| !l.starts_with("msgstr") && !l.starts_with("msgid_plural"))
                        .collect();
                    let ctxt = block.lines().find(|l| l.starts_with("msgctxt "));
                    (!id.is_empty()).then(|| format!("{ctxt:?}{}", id.join("")))
                })
                .collect()
        };
        let pot_ids = ids(pot);
        let de_ids = ids(de);
        // Kopf (msgid "") plus mindestens ein Text.
        assert!(pot_ids.len() >= 2, "Vorlage zu klein: {}", pot_ids.len());
        for id in &pot_ids {
            assert!(de_ids.contains(id), "fehlt in de.po: {id}");
        }
        for block in de.split("\n\n").skip(1) {
            let empty = block
                .lines()
                .any(|l| l.starts_with("msgstr") && l.ends_with(" \"\""))
                && !block.lines().any(|l| l.starts_with('"'));
            assert!(!empty, "unübersetzt in de.po:\n{block}");
        }
    }

    #[test]
    fn named_placeholders_are_filled() {
        let msgid = "{ungelesen} of {anzahl}";
        assert_eq!(
            gettext_f(msgid, &[("anzahl", "7"), ("ungelesen", "2")]),
            "2 of 7"
        );
    }

    #[test]
    fn catalogs_are_searched_beside_the_binary_then_in_the_prefix() {
        let tmp = std::env::temp_dir().join(format!("qb-i18n-{}", std::process::id()));
        let ablage = tmp.join("ablage");
        std::fs::create_dir_all(ablage.join("share/locale")).expect("Testordner");
        assert_eq!(locale_dir_for(Some(&ablage)), ablage.join("share/locale"));
        let bin = tmp.join("usr/bin");
        std::fs::create_dir_all(&bin).expect("Testordner");
        std::fs::create_dir_all(tmp.join("usr/share/locale")).expect("Testordner");
        assert_eq!(locale_dir_for(Some(&bin)), tmp.join("usr/share/locale"));
        let leer = tmp.join("leer/bin");
        std::fs::create_dir_all(&leer).expect("Testordner");
        assert_eq!(
            locale_dir_for(Some(&leer)),
            PathBuf::from("/usr/share/locale")
        );
        assert_eq!(locale_dir_for(None), PathBuf::from("/usr/share/locale"));
        std::fs::remove_dir_all(&tmp).expect("Testordner");
    }
}
