//! Symbol einer Meldung: Reihenfolge nach Spezifikation `image-data` >
//! `image-path` > `app_icon` > Symbol aus dem `.desktop`-Eintrag; sonst ein
//! Standardsymbol nach Dringlichkeit. Pfade, `file://`-URIs und
//! Themensymbolnamen werden unterschieden.

use std::path::{Path, PathBuf};

use gtk4::prelude::*;
use gtk4::{gdk, glib};

use crate::modell::{BildDaten, Dringlichkeit, Meldung};

pub const GROESSE: i32 = 48;

/// Das Symbol-Widget für eine Meldung, immer `GROESSE` × `GROESSE`.
pub fn widget(m: &Meldung) -> gtk4::Widget {
    if let Some(w) = m.bild_daten.as_ref().and_then(aus_daten) {
        return w;
    }
    for angabe in [m.bild_pfad.as_deref(), Some(m.app_icon.as_str())]
        .into_iter()
        .flatten()
    {
        if let Some(w) = aus_angabe(angabe) {
            return w;
        }
    }
    if let Some(w) = m
        .desktop_entry
        .as_deref()
        .and_then(|id| desktop_symbol(id, &datenverzeichnisse()))
        .and_then(|name| aus_angabe(&name))
    {
        return w;
    }
    let name = match m.dringlichkeit {
        Dringlichkeit::Kritisch => "dialog-warning",
        _ => "dialog-information",
    };
    themensymbol(name).unwrap_or_else(|| {
        themensymbol("image-missing").unwrap_or_else(|| gtk4::Image::new().upcast())
    })
}

fn aus_daten(b: &BildDaten) -> Option<gtk4::Widget> {
    if !b.plausibel() {
        return None;
    }
    let format = if b.alpha {
        gdk::MemoryFormat::R8g8b8a8
    } else {
        gdk::MemoryFormat::R8g8b8
    };
    let bytes = glib::Bytes::from(&b.daten[..]);
    let textur =
        gdk::MemoryTexture::new(b.breite, b.hoehe, format, &bytes, b.zeilenlaenge as usize);
    Some(bild(textur.upcast::<gdk::Paintable>()))
}

/// Pfad, `file://`-URI oder Themensymbolname.
fn aus_angabe(angabe: &str) -> Option<gtk4::Widget> {
    let angabe = angabe.trim();
    if angabe.is_empty() {
        return None;
    }
    match pfad_aus_angabe(angabe) {
        Some(pfad) => gdk::Texture::from_filename(&pfad)
            .ok()
            .map(|t| bild(t.upcast())),
        None => themensymbol(angabe),
    }
}

/// Deutet `angabe` als Dateipfad: absolute Pfade und `file://`-URIs; alles
/// andere ist ein Symbolname.
pub fn pfad_aus_angabe(angabe: &str) -> Option<PathBuf> {
    if let Some(rest) = angabe.strip_prefix("file://") {
        return glib::filename_from_uri(angabe)
            .map(|(p, _)| p)
            .ok()
            .or_else(|| Some(PathBuf::from(rest)));
    }
    if angabe.starts_with('/') || angabe.starts_with("~/") {
        let p = if let Some(rest) = angabe.strip_prefix("~/") {
            glib::home_dir().join(rest)
        } else {
            PathBuf::from(angabe)
        };
        return Some(p);
    }
    None
}

fn themensymbol(name: &str) -> Option<gtk4::Widget> {
    let display = gdk::Display::default()?;
    let thema = gtk4::IconTheme::for_display(&display);
    // Ein Name mit Endung (`foo.png`, von notify-send -i) wird ohne Endung gesucht.
    let name = name
        .strip_suffix(".png")
        .or_else(|| name.strip_suffix(".svg"))
        .unwrap_or(name);
    if !thema.has_icon(name) {
        return None;
    }
    let image = gtk4::Image::from_icon_name(name);
    image.set_pixel_size(GROESSE);
    image.set_valign(gtk4::Align::Start);
    Some(image.upcast())
}

fn bild(paintable: gdk::Paintable) -> gtk4::Widget {
    let picture = gtk4::Picture::for_paintable(&paintable);
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk4::ContentFit::Contain);
    picture.set_size_request(GROESSE, GROESSE);
    picture.set_halign(gtk4::Align::Center);
    picture.set_valign(gtk4::Align::Start);
    picture.set_hexpand(false);
    picture.set_vexpand(false);
    picture.upcast()
}

/// XDG-Datenverzeichnisse: `$XDG_DATA_HOME` und `$XDG_DATA_DIRS`.
pub fn datenverzeichnisse() -> Vec<PathBuf> {
    let mut v = vec![glib::user_data_dir()];
    v.extend(glib::system_data_dirs());
    v
}

/// Liest `Icon=` aus `applications/<id>.desktop` in den Datenverzeichnissen.
pub fn desktop_symbol(id: &str, verzeichnisse: &[PathBuf]) -> Option<String> {
    let id = id.trim();
    if id.is_empty() || id.contains('/') || id.contains("..") {
        return None;
    }
    let datei = if id.ends_with(".desktop") {
        id.to_string()
    } else {
        format!("{id}.desktop")
    };
    verzeichnisse
        .iter()
        .find_map(|v| symbol_aus_desktop_datei(&v.join("applications").join(&datei)))
}

fn symbol_aus_desktop_datei(pfad: &Path) -> Option<String> {
    let kf = glib::KeyFile::new();
    kf.load_from_file(pfad, glib::KeyFileFlags::NONE).ok()?;
    let name = kf.string("Desktop Entry", "Icon").ok()?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pfadangaben() {
        assert_eq!(
            pfad_aus_angabe("/usr/share/x.png"),
            Some(PathBuf::from("/usr/share/x.png"))
        );
        assert_eq!(
            pfad_aus_angabe("file:///tmp/a%20b.png"),
            Some(PathBuf::from("/tmp/a b.png"))
        );
        assert_eq!(pfad_aus_angabe("dialog-information"), None);
        assert_eq!(pfad_aus_angabe("org.querencia.Editor"), None);
        assert_eq!(
            pfad_aus_angabe("~/bild.png"),
            Some(glib::home_dir().join("bild.png"))
        );
    }

    #[test]
    fn desktop_symbol_aus_datei() {
        let dir = std::env::temp_dir().join(format!("qb-desktop-{}", std::process::id()));
        let apps = dir.join("applications");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::write(
            apps.join("org.test.Probe.desktop"),
            "[Desktop Entry]\nType=Application\nName=Probe\nIcon=  probe-symbol \nExec=true\n",
        )
        .unwrap();
        std::fs::write(
            apps.join("ohne.desktop"),
            "[Desktop Entry]\nType=Application\nName=Ohne\nExec=true\n",
        )
        .unwrap();
        let v = vec![PathBuf::from("/nirgends"), dir.clone()];
        assert_eq!(
            desktop_symbol("org.test.Probe", &v).as_deref(),
            Some("probe-symbol")
        );
        assert_eq!(
            desktop_symbol("org.test.Probe.desktop", &v).as_deref(),
            Some("probe-symbol")
        );
        assert_eq!(desktop_symbol("ohne", &v), None);
        assert_eq!(desktop_symbol("fehlt", &v), None);
        assert_eq!(desktop_symbol("../etc/passwd", &v), None);
        assert_eq!(desktop_symbol("", &v), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
