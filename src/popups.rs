//! Die Popups: ein einziges Layer-Shell-Fenster (Ebene overlay, oben rechts,
//! ohne Tastatur) mit einer senkrechten Spalte von höchstens `MAX_SICHTBAR`
//! Meldungskästen; weitere Meldungen warten in einer Schlange. Das Fenster
//! setzt die Exklusivzone 0, dadurch legt der Compositor es unter das obere
//! Panel (dessen Exklusivzone), ohne dass die Panelhöhe geraten wird.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use gtk4::prelude::*;
use gtk4::{gdk, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use zbus::Connection;
use zbus::object_server::SignalEmitter;

use crate::dbus::{self, Notifications, Querencia, grund};
use crate::modell::{Dringlichkeit, Meldung, Zustand};
use crate::{markup, symbole};

pub const MAX_SICHTBAR: usize = 5;
/// Breite eines Meldungskastens in Pixeln.
pub const BREITE: i32 = 360;
const NAMESPACE: &str = "querencia-benachrichtigungen";

const CSS: &str = r#"
window.qb-stapel { background: transparent; }
box.qb-meldung {
  background-color: #f9f8f6;
  border: 1px solid #a8a39e;
  padding: 8px 8px 8px 10px;
  color: #2e2e2e;
}
box.qb-meldung.qb-kritisch { border: 2px solid #f07746; padding: 7px 7px 7px 9px; }
label.qb-betreff { font-weight: bold; }
label.qb-text { color: #3c3c3c; }
label.qb-app { color: #7a756f; font-size: 0.85em; }
button.qb-schliessen {
  padding: 0; min-width: 18px; min-height: 18px;
  background: none; border: none; box-shadow: none; -gtk-icon-size: 12px;
}
button.qb-schliessen:hover { background-color: #e5e1dd; }
button.qb-aktion { padding: 1px 10px; min-height: 22px; border-radius: 2px; }
"#;

thread_local! {
    static DIENST: RefCell<Option<Dienst>> = const { RefCell::new(None) };
}

/// Führt `f` mit dem Dienst des GTK-Threads aus (nach `einrichten`). Ist der
/// Dienst gerade in Arbeit (GTK ruft z. B. beim Entfernen eines Kastens
/// synchron `leave` auf), kommt `None` zurück und nichts geschieht.
pub fn mit_dienst<R>(f: impl FnOnce(&mut Dienst) -> R) -> Option<R> {
    DIENST.with(|d| d.try_borrow_mut().ok().and_then(|mut d| d.as_mut().map(f)))
}

/// Führt `f` im nächsten Leerlauf des GTK-Hauptkreises aus, also außerhalb
/// jedes laufenden Signal-Handlers. Für Klicks, Knöpfe und Zeitgeber.
fn spaeter(f: impl FnOnce(&mut Dienst) + 'static) {
    glib::idle_add_local_once(move || {
        if mit_dienst(f).is_none() {
            dbus::log("Dienst nicht erreichbar (interner Fehler)");
        }
    });
}

pub fn einrichten(conn: Connection, zustand: Arc<Mutex<Zustand>>) -> Result<(), String> {
    let dienst = Dienst::neu(conn, zustand)?;
    DIENST.with(|d| *d.borrow_mut() = Some(dienst));
    Ok(())
}

struct Popup {
    meldung: Meldung,
    kasten: gtk4::Box,
    timer: Option<glib::SourceId>,
    /// Zeiger steht über dem Kasten: Zeit läuft nicht.
    ueberfahren: bool,
}

pub struct Dienst {
    conn: Connection,
    zustand: Arc<Mutex<Zustand>>,
    fenster: gtk4::Window,
    spalte: gtk4::Box,
    sichtbar: Vec<Popup>,
    wartend: VecDeque<Meldung>,
}

impl Dienst {
    fn neu(conn: Connection, zustand: Arc<Mutex<Zustand>>) -> Result<Self, String> {
        let display = gdk::Display::default().ok_or("kein Display")?;
        symbole::klassisches_thema_verwenden();
        let css = gtk4::CssProvider::new();
        css.load_from_string(CSS);
        gtk4::style_context_add_provider_for_display(
            &display,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let spalte = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        spalte.set_margin_top(8);
        spalte.set_margin_end(8);
        let fenster = gtk4::Window::builder()
            .title("Querencia Benachrichtigungen")
            .child(&spalte)
            .decorated(false)
            .resizable(false)
            .build();
        fenster.add_css_class("qb-stapel");
        fenster.init_layer_shell();
        fenster.set_namespace(Some(NAMESPACE));
        fenster.set_layer(Layer::Overlay);
        fenster.set_keyboard_mode(KeyboardMode::None);
        fenster.set_anchor(Edge::Top, true);
        fenster.set_anchor(Edge::Right, true);
        fenster.set_anchor(Edge::Bottom, false);
        fenster.set_anchor(Edge::Left, false);
        // 0: der Compositor rückt das Fenster aus fremden Exklusivzonen (Panel) heraus.
        fenster.set_exclusive_zone(0);
        Ok(Self {
            conn,
            zustand,
            fenster,
            spalte,
            sichtbar: Vec::new(),
            wartend: VecDeque::new(),
        })
    }

    fn emitter(&self) -> Option<SignalEmitter<'static>> {
        SignalEmitter::new(&self.conn, dbus::PFAD).ok()
    }

    fn signal_geschlossen(&self, id: u32, grund: u32) {
        let Some(e) = self.emitter() else { return };
        if let Err(err) = zbus::block_on(Notifications::notification_closed(&e, id, grund)) {
            dbus::log(&format!("NotificationClosed({id}) nicht gesendet: {err}"));
        }
    }

    fn signal_aktion(&self, id: u32, schluessel: &str) {
        let Some(e) = self.emitter() else { return };
        if let Err(err) = zbus::block_on(Notifications::action_invoked(
            &e,
            id,
            schluessel.to_string(),
        )) {
            dbus::log(&format!("ActionInvoked({id}) nicht gesendet: {err}"));
        }
    }

    /// Zähler in den gemeinsamen Zustand schreiben und melden.
    fn zaehler_melden(&self) {
        let tupel = {
            let mut z = self.zustand.lock().unwrap_or_else(|e| e.into_inner());
            z.sichtbar = self.sichtbar.len() as u32;
            z.wartend = self.wartend.len() as u32;
            dbus::zustand_tupel(&z)
        };
        if let Some(e) = self.emitter() {
            let _ = zbus::block_on(Querencia::zustand_geaendert(&e, tupel.0, tupel.1, tupel.2));
        }
    }

    /// Neue oder ersetzende Meldung aufnehmen.
    pub fn zeige(&mut self, meldung: Meldung) {
        let id = meldung.id;
        if let Some(index) = self.sichtbar.iter().position(|p| p.meldung.id == id) {
            // Ersatz einer sichtbaren Meldung an derselben Stelle.
            let alt = &mut self.sichtbar[index];
            alt.meldung = meldung;
            let kasten = baue_kasten(&alt.meldung);
            self.spalte.insert_child_after(&kasten, Some(&alt.kasten));
            self.spalte.remove(&alt.kasten);
            alt.kasten = kasten;
            alt.ueberfahren = false;
            self.timer_stoppen(index);
            self.timer_starten(index);
            dbus::log(&format!(
                "Meldung {id} ersetzt: „{}“",
                self.sichtbar[index].meldung.summary
            ));
            self.zaehler_melden();
            return;
        }
        if let Some(alt) = self.wartend.iter_mut().find(|m| m.id == id) {
            *alt = meldung;
            dbus::log(&format!("wartende Meldung {id} ersetzt"));
            return;
        }
        if self.sichtbar.len() >= MAX_SICHTBAR {
            dbus::log(&format!(
                "Meldung {id} wartet ({} sichtbar)",
                self.sichtbar.len()
            ));
            self.wartend.push_back(meldung);
            self.zaehler_melden();
            return;
        }
        self.anzeigen(meldung);
        self.zaehler_melden();
    }

    fn anzeigen(&mut self, meldung: Meldung) {
        let kasten = baue_kasten(&meldung);
        self.spalte.append(&kasten);
        dbus::log(&format!(
            "Meldung {} von „{}“ ({}{}): „{}“",
            meldung.id,
            meldung.app_name,
            meldung.dringlichkeit.name(),
            meldung
                .category
                .as_deref()
                .map(|c| format!(", {c}"))
                .unwrap_or_default(),
            meldung.summary
        ));
        self.sichtbar.push(Popup {
            meldung,
            kasten,
            timer: None,
            ueberfahren: false,
        });
        let index = self.sichtbar.len() - 1;
        self.timer_starten(index);
        if !self.fenster.is_visible() {
            self.fenster.present();
        }
    }

    fn timer_starten(&mut self, index: usize) {
        let Some(p) = self.sichtbar.get_mut(index) else {
            return;
        };
        if p.ueberfahren || p.timer.is_some() {
            return;
        }
        let Some(dauer) = p.meldung.anzeigedauer() else {
            return;
        };
        let id = p.meldung.id;
        p.timer = Some(glib::timeout_add_local_once(dauer, move || {
            spaeter(move |d| {
                if let Some(p) = d.sichtbar.iter_mut().find(|p| p.meldung.id == id) {
                    p.timer = None;
                }
                d.schliesse(id, grund::ABGELAUFEN);
            });
        }));
    }

    fn timer_stoppen(&mut self, index: usize) {
        if let Some(t) = self.sichtbar.get_mut(index).and_then(|p| p.timer.take()) {
            t.remove();
        }
    }

    fn ueberfahren(&mut self, id: u32, drin: bool) {
        let Some(index) = self.sichtbar.iter().position(|p| p.meldung.id == id) else {
            return;
        };
        self.sichtbar[index].ueberfahren = drin;
        if drin {
            self.timer_stoppen(index);
        } else {
            self.timer_starten(index);
        }
    }

    /// Schließt eine sichtbare oder wartende Meldung und meldet den Grund.
    pub fn schliesse(&mut self, id: u32, grund: u32) {
        let mut gefunden = false;
        if let Some(index) = self.sichtbar.iter().position(|p| p.meldung.id == id) {
            self.timer_stoppen(index);
            let p = self.sichtbar.remove(index);
            self.spalte.remove(&p.kasten);
            gefunden = true;
        } else if let Some(index) = self.wartend.iter().position(|m| m.id == id) {
            self.wartend.remove(index);
            gefunden = true;
        }
        if !gefunden {
            return;
        }
        self.zustand
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .aktiv
            .remove(&id);
        dbus::log(&format!("Meldung {id} geschlossen (Grund {grund})"));
        self.signal_geschlossen(id, grund);
        self.nachruecken();
        if self.sichtbar.is_empty() {
            self.fenster.set_visible(false);
        }
        self.zaehler_melden();
    }

    fn nachruecken(&mut self) {
        while self.sichtbar.len() < MAX_SICHTBAR {
            let Some(m) = self.wartend.pop_front() else {
                break;
            };
            self.anzeigen(m);
        }
    }

    fn aktion(&mut self, id: u32, schluessel: &str) {
        let Some(p) = self.sichtbar.iter().find(|p| p.meldung.id == id) else {
            return;
        };
        let resident = p.meldung.resident;
        dbus::log(&format!("Meldung {id}: Aktion „{schluessel}“"));
        self.signal_aktion(id, schluessel);
        if !resident {
            self.schliesse(id, grund::GESCHLOSSEN);
        }
    }

    /// Klick auf den Kasten: Default-Aktion, sonst schließen.
    fn klick(&mut self, id: u32) {
        let Some(p) = self.sichtbar.iter().find(|p| p.meldung.id == id) else {
            return;
        };
        if p.meldung.hat_default_aktion() {
            self.aktion(id, "default");
        } else {
            self.schliesse(id, grund::GESCHLOSSEN);
        }
    }
}

fn baue_kasten(m: &Meldung) -> gtk4::Box {
    let id = m.id;
    let kasten = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    kasten.add_css_class("qb-meldung");
    if m.dringlichkeit == Dringlichkeit::Kritisch {
        kasten.add_css_class("qb-kritisch");
    }
    kasten.set_size_request(BREITE, -1);
    kasten.set_hexpand(false);

    kasten.append(&symbole::widget(m));

    let inhalt = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    inhalt.set_hexpand(true);
    inhalt.set_size_request(BREITE - symbole::GROESSE - 40, -1);

    let kopf = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let betreff = gtk4::Label::new(Some(if m.summary.is_empty() {
        &m.app_name
    } else {
        &m.summary
    }));
    betreff.add_css_class("qb-betreff");
    betreff.set_xalign(0.0);
    betreff.set_hexpand(true);
    betreff.set_wrap(true);
    // WordChar bricht Wörter ohne Leerzeichen hart um; 2 Zeilen, dann Ellipse.
    betreff.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    betreff.set_width_chars(10);
    betreff.set_max_width_chars(26);
    betreff.set_lines(2);
    betreff.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    kopf.append(&betreff);
    let schliessen = gtk4::Button::from_icon_name("window-close-symbolic");
    schliessen.add_css_class("qb-schliessen");
    schliessen.set_valign(gtk4::Align::Start);
    schliessen.set_tooltip_text(Some("Schließen"));
    schliessen.set_can_focus(false);
    schliessen.connect_clicked(move |_| spaeter(move |d| d.schliesse(id, grund::GESCHLOSSEN)));
    kopf.append(&schliessen);
    inhalt.append(&kopf);

    if !m.body.is_empty() {
        let text = gtk4::Label::new(None);
        text.add_css_class("qb-text");
        text.set_xalign(0.0);
        text.set_wrap(true);
        text.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        text.set_width_chars(10);
        text.set_max_width_chars(30);
        // Höchstens 5 Zeilen (3 mit Aktionsknöpfen), damit der Kasten unter
        // etwa 160 px bleibt; der Rest wird mit Ellipse abgeschnitten.
        text.set_lines(if m.knopf_aktionen().next().is_some() {
            3
        } else {
            5
        });
        text.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        text.set_selectable(false);
        let markup = markup::bereinige(&m.body);
        if gtk4::pango::parse_markup(&markup::pango_pruefbar(&markup), '\0').is_ok() {
            text.set_markup(&markup);
        } else {
            text.set_text(&m.body);
        }
        inhalt.append(&text);
    }

    if !m.app_name.is_empty() && !m.summary.is_empty() {
        let app = gtk4::Label::new(Some(&m.app_name));
        app.add_css_class("qb-app");
        app.set_xalign(0.0);
        app.set_width_chars(10);
        app.set_max_width_chars(30);
        app.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        inhalt.append(&app);
    }

    let knoepfe: Vec<_> = m.knopf_aktionen().collect();
    if !knoepfe.is_empty() {
        let reihe = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        reihe.set_halign(gtk4::Align::End);
        reihe.set_margin_top(4);
        for (schluessel, beschriftung) in knoepfe {
            let knopf = gtk4::Button::with_label(beschriftung);
            knopf.add_css_class("qb-aktion");
            knopf.set_can_focus(false);
            let schluessel = schluessel.clone();
            knopf.connect_clicked(move |_| {
                let schluessel = schluessel.clone();
                spaeter(move |d| d.aktion(id, &schluessel));
            });
            reihe.append(&knopf);
        }
        inhalt.append(&reihe);
    }
    kasten.append(&inhalt);

    // Zeiger über dem Kasten hält die Zeit an.
    let bewegung = gtk4::EventControllerMotion::new();
    bewegung.connect_enter(move |_, _, _| {
        mit_dienst(|d| d.ueberfahren(id, true));
    });
    bewegung.connect_leave(move |_| {
        mit_dienst(|d| d.ueberfahren(id, false));
    });
    kasten.add_controller(bewegung);

    // Klick auf die Meldung selbst: Default-Aktion. Knöpfe (Schließen,
    // Aktionen) behandeln ihren Klick selbst und werden hier übersprungen.
    let klick = gtk4::GestureClick::new();
    klick.set_button(gdk::BUTTON_PRIMARY);
    klick.set_propagation_phase(gtk4::PropagationPhase::Bubble);
    klick.connect_released(move |geste, _, x, y| {
        let ziel = geste
            .widget()
            .and_then(|w| w.pick(x, y, gtk4::PickFlags::DEFAULT));
        let auf_knopf = ziel.as_ref().is_some_and(|w| {
            w.is::<gtk4::Button>() || w.ancestor(gtk4::Button::static_type()).is_some()
        });
        if std::env::var_os("QB_DEBUG").is_some() {
            dbus::log(&format!(
                "Klick bei {x:.0},{y:.0} auf {} (Knopf: {auf_knopf})",
                ziel.as_ref()
                    .map(|w| w.type_().name().to_string())
                    .unwrap_or_else(|| "nichts".into())
            ));
        }
        if auf_knopf {
            return;
        }
        geste.set_state(gtk4::EventSequenceState::Claimed);
        spaeter(move |d| d.klick(id));
    });
    kasten.add_controller(klick);
    kasten
}
