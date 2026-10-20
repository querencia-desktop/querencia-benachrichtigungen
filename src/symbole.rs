//! Symbol einer Meldung: Reihenfolge nach Spezifikation `image-data` >
//! `image-path` > `app_icon` > Symbol aus dem `.desktop`-Eintrag; sonst ein
//! Standardsymbol nach Dringlichkeit. Pfade, `file://`-URIs und
//! Themensymbolnamen werden unterschieden.

use std::path::{Path, PathBuf};

use gtk4::prelude::*;
use gtk4::{gdk, gdk_pixbuf, glib};

use crate::modell::{BildDaten, Dringlichkeit, Meldung};
use crate::verlauf::Eintrag;

pub const GROESSE: i32 = 48;

/// Symbolthema des GNOME-2-Looks, dieselbe Regel wie im Querencia Panel
/// (`classic_icon_theme` in querencia-panel/src/main.rs): nur wenn das
/// aktuelle Thema das eingebaute GTK-Standardthema ist und gnome-human
/// installiert ist. Adwaita 50 hat keine farbigen Statussymbole mehr.
const KLASSISCHES_THEMA: &str = "gnome-human";

pub fn klassisches_thema(
    aktuell: &str,
    installiert: impl Fn(&str) -> bool,
) -> Option<&'static str> {
    let eingebaut = matches!(aktuell, "" | "Adwaita" | "hicolor");
    (eingebaut && installiert(KLASSISCHES_THEMA)).then_some(KLASSISCHES_THEMA)
}

fn thema_installiert(name: &str, verzeichnisse: &[PathBuf]) -> bool {
    verzeichnisse
        .iter()
        .any(|v| v.join("icons").join(name).join("index.theme").is_file())
}

/// Schaltet diesen Prozess auf das klassische Symbolthema, wo
/// [`klassisches_thema`] es sagt. Nur die Einstellung dieses Prozesses.
pub fn klassisches_thema_verwenden() {
    let Some(settings) = gtk4::Settings::default() else {
        return;
    };
    let aktuell = settings
        .gtk_icon_theme_name()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let verzeichnisse = datenverzeichnisse();
    match klassisches_thema(&aktuell, |n| thema_installiert(n, &verzeichnisse)) {
        Some(thema) => {
            settings.set_gtk_icon_theme_name(Some(thema));
            crate::dbus::log(&format!("Symbolthema {thema} (statt „{aktuell}“)"));
        }
        None => crate::dbus::log(&format!("Symbolthema „{aktuell}“ bleibt")),
    }
}

/// Das Symbol-Widget für eine Meldung, immer `GROESSE` × `GROESSE`.
pub fn widget(m: &Meldung) -> gtk4::Widget {
    widget_aus(
        m.bild_daten.as_ref(),
        &[m.bild_pfad.as_deref().unwrap_or(""), m.app_icon.as_str()],
        m.desktop_entry.as_deref(),
        m.dringlichkeit,
        GROESSE,
    )
}

/// Symbol eines Verlaufseintrags in der Zentrale, `groesse` × `groesse`.
pub fn widget_eintrag(e: &Eintrag, groesse: i32) -> gtk4::Widget {
    widget_aus(
        e.bild_daten.as_ref(),
        &[e.bild_pfad.as_str(), e.app_icon.as_str()],
        Some(e.desktop_entry.as_str()).filter(|d| !d.is_empty()),
        e.dringlichkeit(),
        groesse,
    )
}

/// Reihenfolge: Bilddaten, Angaben (Pfad, URI oder Themenname), Symbol des
/// `.desktop`-Eintrags, Standardsymbol nach Dringlichkeit.
fn widget_aus(
    daten: Option<&BildDaten>,
    angaben: &[&str],
    desktop_entry: Option<&str>,
    dringlichkeit: Dringlichkeit,
    groesse: i32,
) -> gtk4::Widget {
    if let Some(w) = daten.and_then(|b| aus_daten(b, groesse)) {
        return w;
    }
    for angabe in angaben {
        if let Some(w) = aus_angabe(angabe, groesse) {
            return w;
        }
    }
    if let Some(w) = desktop_entry
        .and_then(|id| desktop_symbol(id, &datenverzeichnisse()))
        .and_then(|name| aus_angabe(&name, groesse))
    {
        return w;
    }
    let name = match dringlichkeit {
        Dringlichkeit::Kritisch => "dialog-warning",
        _ => "dialog-information",
    };
    themensymbol(name, groesse).unwrap_or_else(|| {
        themensymbol("image-missing", groesse).unwrap_or_else(|| gtk4::Image::new().upcast())
    })
}

fn aus_daten(b: &BildDaten, groesse: i32) -> Option<gtk4::Widget> {
    if !b.plausibel() {
        return None;
    }
    // Über GdkPixbuf, damit das Bild vor der Anzeige auf GROESSE verkleinert
    // wird: Electron/libnotify schicken das Programmsymbol oft als 256×256
    // oder größer, und ein GtkPicture würde den Kasten darauf aufblasen.
    let bytes = glib::Bytes::from(&b.daten[..]);
    let pixbuf = gdk_pixbuf::Pixbuf::from_bytes(
        &bytes,
        gdk_pixbuf::Colorspace::Rgb,
        b.alpha,
        b.bits_je_kanal,
        b.breite,
        b.hoehe,
        b.zeilenlaenge,
    );
    verkleinert(&pixbuf, groesse).map(|p| bild(p, groesse))
}

/// Verkleinert auf höchstens `groesse` × `groesse` (Seitenverhältnis
/// bleibt); kleinere Bilder bleiben, wie sie sind.
fn verkleinert(pixbuf: &gdk_pixbuf::Pixbuf, groesse: i32) -> Option<gdk::Paintable> {
    let (b, h) = (pixbuf.width(), pixbuf.height());
    if b <= 0 || h <= 0 {
        return None;
    }
    let skaliert = if b > groesse || h > groesse {
        let (zb, zh) = zielmasse(b, h, groesse);
        pixbuf.scale_simple(zb, zh, gdk_pixbuf::InterpType::Bilinear)?
    } else {
        pixbuf.clone()
    };
    Some(gdk::Texture::for_pixbuf(&skaliert).upcast())
}

/// Zielmaße für ein Bild b×h, das in ein Quadrat `max` passen soll.
pub fn zielmasse(b: i32, h: i32, max: i32) -> (i32, i32) {
    if b >= h {
        (max, (h as i64 * max as i64 / b as i64).max(1) as i32)
    } else {
        ((b as i64 * max as i64 / h as i64).max(1) as i32, max)
    }
}

/// Pfad, `file://`-URI oder Themensymbolname.
fn aus_angabe(angabe: &str, groesse: i32) -> Option<gtk4::Widget> {
    let angabe = angabe.trim();
    if angabe.is_empty() {
        return None;
    }
    match pfad_aus_angabe(angabe) {
        // Beim Laden schon auf die Größe begrenzen (auch große PNGs/SVGs).
        Some(pfad) => gdk_pixbuf::Pixbuf::from_file_at_scale(&pfad, groesse, groesse, true)
            .ok()
            .and_then(|p| verkleinert(&p, groesse))
            .map(|p| bild(p, groesse)),
        None => themensymbol(angabe, groesse),
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

fn themensymbol(name: &str, groesse: i32) -> Option<gtk4::Widget> {
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
    image.set_pixel_size(groesse);
    image.set_valign(gtk4::Align::Start);
    Some(image.upcast())
}

fn bild(paintable: gdk::Paintable, groesse: i32) -> gtk4::Widget {
    let picture = gtk4::Picture::for_paintable(&paintable);
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk4::ContentFit::Contain);
    picture.set_size_request(groesse, groesse);
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
    fn zielmasse_halten_das_seitenverhaeltnis() {
        assert_eq!(zielmasse(512, 512, 48), (48, 48));
        assert_eq!(zielmasse(256, 128, 48), (48, 24));
        assert_eq!(zielmasse(100, 400, 48), (12, 48));
        assert_eq!(zielmasse(4096, 1, 48), (48, 1));
    }

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
    fn klassisches_thema_nur_statt_eingebautem_standard() {
        let installiert = |n: &str| n == "gnome-human";
        for aktuell in ["Adwaita", "hicolor", ""] {
            assert_eq!(
                klassisches_thema(aktuell, installiert),
                Some("gnome-human"),
                "{aktuell:?}"
            );
        }
        assert_eq!(
            klassisches_thema("Humanity", installiert),
            None,
            "vom Benutzer gewählt bleibt"
        );
        assert_eq!(klassisches_thema("gnome-human", installiert), None);
        assert_eq!(
            klassisches_thema("Adwaita", |_| false),
            None,
            "nicht installiert"
        );
    }

    #[test]
    fn thema_installiert_prueft_index_theme() {
        let dir = std::env::temp_dir().join(format!("qb-thema-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("icons/gnome-human")).unwrap();
        assert!(!thema_installiert(
            "gnome-human",
            std::slice::from_ref(&dir)
        ));
        std::fs::write(dir.join("icons/gnome-human/index.theme"), "[Icon Theme]\n").unwrap();
        assert!(thema_installiert("gnome-human", std::slice::from_ref(&dir)));
        assert!(!thema_installiert(
            "gnome-human",
            &[PathBuf::from("/nirgends")]
        ));
        let _ = std::fs::remove_dir_all(&dir);
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
