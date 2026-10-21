//! Die Popups: ein einziges Layer-Shell-Fenster (Ebene overlay, ohne
//! Tastatur; oben rechts, oben in der Mitte oder unten rechts nach der ini)
//! mit einer senkrechten Spalte von höchstens `MAX_SICHTBAR` Meldungskästen;
//! weitere Meldungen warten in einer Schlange. Das Fenster setzt die
//! Exklusivzone 0, dadurch legt der Compositor es neben die Panels (deren
//! Exklusivzonen), ohne dass die Panelhöhe geraten wird.
//!
//! Der `Dienst` des GTK-Threads hält auch die Mitteilungszentrale
//! (`zentrale.rs`) und führt alles aus, was dort angeklickt wird.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use gtk4::prelude::*;
use gtk4::{gdk, glib};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use zbus::Connection;
use zbus::object_server::SignalEmitter;

use crate::dbus::{self, Notifications, Querencia, grund};
use crate::einstellung::Position;
use crate::i18n;
use crate::modell::{Dringlichkeit, Meldung, Zustand};
use crate::zentrale::Zentrale;
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
label.qb-app { color: #6f6a64; font-size: 0.85em; }
button.qb-schliessen {
  padding: 0; min-width: 18px; min-height: 18px;
  background: none; border: none; box-shadow: none; -gtk-icon-size: 12px;
}
button.qb-schliessen:hover { background-color: #e5e1dd; }
button.qb-aktion { padding: 1px 10px; min-height: 22px; border-radius: 2px; }
button.qb-hinweis {
  padding: 0 2px; min-height: 0; min-width: 0;
  background: none; border: none; box-shadow: none;
  color: #6f6a64; font-size: 0.85em;
}
button.qb-hinweis:hover { text-decoration: underline; color: #3c3c3c; }
@media (prefers-color-scheme: dark) {
  box.qb-meldung { background-color: #34332f; border-color: #23221f; color: #f2f1f0; }
  label.qb-text { color: #dfdbd2; }
  label.qb-app, button.qb-hinweis { color: #b3aea7; }
  button.qb-schliessen:hover { background-color: #4a4843; }
  button.qb-hinweis:hover { color: #dfdbd2; }
}
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
pub fn spaeter(f: impl FnOnce(&mut Dienst) + 'static) {
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
    /// Fenster der Mitteilungszentrale, beim ersten Zeigen gebaut.
    zentrale: Option<Zentrale>,
}

impl Dienst {
    fn neu(conn: Connection, zustand: Arc<Mutex<Zustand>>) -> Result<Self, String> {
        let display = gdk::Display::default().ok_or("kein Display")?;
        symbole::klassisches_thema_verwenden();
        let css = gtk4::CssProvider::new();
        // Der Dunkelblock in CSS greift nur, wenn dieser eigene Anbieter dem
        // Farbschema folgt; GTK bindet von sich aus nur den Themenanbieter.
        // Gemessen in 0.2.1: ohne Bindung blieben die Popups beim Start und
        // nach jedem Wechsel hell.
        if let Some(s) = gtk4::Settings::default() {
            s.bind_property("gtk-interface-color-scheme", &css, "prefers-color-scheme")
                .sync_create()
                .build();
        }
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
            zentrale: None,
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

    /// Zähler in den gemeinsamen Zustand schreiben und `ZustandGeaendert`
    /// melden.
    pub fn zaehler_melden(&self) {
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
            let hinweis = self.hinweis();
            let alt = &mut self.sichtbar[index];
            alt.meldung = meldung;
            let kasten = baue_kasten(&alt.meldung, hinweis);
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

    /// Ob Popups den Hinweis „in der Zentrale“ tragen (ini `hinweis=`).
    fn hinweis(&self) -> bool {
        self.zustand
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .ein()
            .hinweis
    }

    /// Anker und Ränder nach `[Einblendung] position=`; nur solange das
    /// Fenster versteckt ist (sonst springt der Stapel).
    fn position_setzen(&self) {
        let position = self
            .zustand
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .ein()
            .position;
        let (oben, unten, rechts) = match position {
            Position::ObenRechts => (true, false, true),
            Position::ObenMitte => (true, false, false),
            Position::UntenRechts => (false, true, true),
        };
        self.fenster.set_anchor(Edge::Top, oben);
        self.fenster.set_anchor(Edge::Bottom, unten);
        self.fenster.set_anchor(Edge::Right, rechts);
        self.fenster.set_anchor(Edge::Left, false);
        self.spalte.set_margin_top(if oben { 8 } else { 0 });
        self.spalte.set_margin_bottom(if unten { 8 } else { 0 });
        self.spalte.set_margin_end(if rechts { 8 } else { 0 });
    }

    fn anzeigen(&mut self, meldung: Meldung) {
        let kasten = baue_kasten(&meldung, self.hinweis());
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
            self.position_setzen();
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
        let dauer_s = self
            .zustand
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .ein()
            .dauer_s;
        let Some(p) = self.sichtbar.get_mut(index) else {
            return;
        };
        let Some(dauer) = p.meldung.anzeigedauer(dauer_s) else {
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
        self.gelesen(vec![id]);
    }

    /// Schließen-Knopf im Popup: zu (Grund 2) und als gelesen.
    fn weggeklickt(&mut self, id: u32) {
        self.schliesse(id, grund::GESCHLOSSEN);
        self.gelesen(vec![id]);
    }

    /// Eine Meldung, die nicht erscheinen darf (Nicht stören, Einblendungen
    /// aus): sofort `NotificationClosed(Grund 1)`, damit Absender wie
    /// `notify-send --wait` nicht ewig warten. Steht ein älteres Popup mit
    /// derselben ID (Ersatz), schließt es sich.
    pub fn unterdrueckt(&mut self, id: u32) {
        let da = self.sichtbar.iter().any(|p| p.meldung.id == id)
            || self.wartend.iter().any(|m| m.id == id);
        if da {
            self.schliesse(id, grund::ABGELAUFEN);
        } else {
            self.signal_geschlossen(id, grund::ABGELAUFEN);
        }
    }

    // ------------------------------------------- Mitteilungszentrale

    /// Nach Änderungen am Verlauf von der GTK-Seite: `Geaendert`,
    /// `Zaehler` (mit `PropertiesChanged`) und die Zentrale auffrischen.
    fn verlauf_gemeldet(&mut self, ids: Vec<u32>) {
        let conn = self.conn.clone();
        let zustand = self.zustand.clone();
        zbus::block_on(async {
            dbus::melde_geaendert(&conn, ids).await;
            dbus::melde_zaehler(&conn, &zustand).await;
        });
        self.zentrale_auffrischen();
    }

    /// Markiert `ids` (leer: alle) als gelesen.
    fn gelesen(&mut self, ids: Vec<u32>) {
        let geaendert = {
            let mut z = self.zustand.lock().unwrap_or_else(|e| e.into_inner());
            let g = z.verlauf.als_gelesen(&ids);
            if !g.is_empty() {
                z.verlauf_sichern();
            }
            g
        };
        if !geaendert.is_empty() {
            self.verlauf_gemeldet(geaendert);
        }
    }

    pub fn zentrale_als_gelesen(&mut self, ids: Vec<u32>) {
        self.gelesen(ids);
    }

    pub fn zentrale_entfernen(&mut self, ids: Vec<u32>) {
        let weg = {
            let mut z = self.zustand.lock().unwrap_or_else(|e| e.into_inner());
            let w = z.verlauf.entfernen(&ids);
            if !w.is_empty() {
                z.verlauf_sichern();
            }
            w
        };
        if weg.is_empty() {
            return;
        }
        dbus::log(&format!(
            "Zentrale: {} Eintrag/Einträge entfernt",
            weg.len()
        ));
        for id in &weg {
            self.schliesse(*id, grund::GESCHLOSSEN);
        }
        self.verlauf_gemeldet(weg);
    }

    pub fn zentrale_alle_entfernen(&mut self) {
        let weg: Vec<u32> = {
            let mut z = self.zustand.lock().unwrap_or_else(|e| e.into_inner());
            if z.verlauf.is_empty() {
                return;
            }
            let ids = z.verlauf.eintraege().map(|e| e.id).collect();
            z.verlauf.leeren();
            z.verlauf_sichern();
            ids
        };
        dbus::log(&format!("Zentrale: alle {} Einträge entfernt", weg.len()));
        for id in &weg {
            self.schliesse(*id, grund::GESCHLOSSEN);
        }
        self.verlauf_gemeldet(Vec::new());
    }

    /// Aktionsknopf in der Zentrale. Steht das Popup noch, gilt dasselbe wie
    /// dort; sonst nur `ActionInvoked` (der Absender hört evtl. noch zu).
    pub fn zentrale_aktion(&mut self, id: u32, schluessel: &str) {
        if self.sichtbar.iter().any(|p| p.meldung.id == id) {
            self.aktion(id, schluessel);
            return;
        }
        dbus::log(&format!("Zentrale: Meldung {id}, Aktion „{schluessel}“"));
        self.signal_aktion(id, schluessel);
        self.gelesen(vec![id]);
    }

    /// Doppelklick oder Eingabe auf einem Eintrag: Standardaktion, wenn es
    /// eine gibt und der Absender noch zuhören kann, sonst „als gelesen“.
    pub fn zentrale_aktivieren(&mut self, id: u32) {
        let default = self
            .zustand
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .verlauf
            .eintrag(id)
            .is_some_and(|e| e.aus_diesem_lauf && e.aktionen.iter().any(|(k, _)| k == "default"));
        if default {
            self.zentrale_aktion(id, "default");
        } else {
            self.gelesen(vec![id]);
        }
    }

    pub fn zentrale_nicht_stoeren(&mut self, an: bool) {
        let conn = self.conn.clone();
        let zustand = self.zustand.clone();
        zbus::block_on(dbus::nicht_stoeren_setzen(&zustand, an, Some(&conn)));
    }

    pub fn zentrale_sichtbarkeit(&mut self, offen: bool) {
        let geaendert = {
            let mut z = self.zustand.lock().unwrap_or_else(|e| e.into_inner());
            let g = z.zentrale_offen != offen;
            z.zentrale_offen = offen;
            g
        };
        if geaendert {
            dbus::log(&format!(
                "Mitteilungszentrale {}",
                if offen { "geöffnet" } else { "geschlossen" }
            ));
            let conn = self.conn.clone();
            zbus::block_on(dbus::melde_zentrale_offen(&conn, offen));
        }
        if offen {
            self.zentrale_auffrischen();
        }
    }

    /// Zeigt die Zentrale (baut sie beim ersten Mal).
    pub fn zentrale_zeigen(&mut self, token: &str) {
        let log = self
            .zustand
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .einstellungen_auffrischen();
        for zeile in log {
            dbus::log(&zeile);
        }
        if self.zentrale.is_none() {
            self.zentrale = Some(Zentrale::neu());
        }
        self.zentrale_fuellen();
        if let Some(z) = &self.zentrale {
            z.zeigen(token);
        }
    }

    /// Das Fenster der Zentrale wurde geschlossen (und zerstört).
    pub fn zentrale_zu(&mut self) {
        self.zentrale = None;
        self.zentrale_sichtbarkeit(false);
    }

    pub fn zentrale_schliessen(&mut self) {
        if let Some(z) = self.zentrale.as_ref().filter(|z| z.sichtbar()) {
            z.fenster.close();
        }
    }

    /// Baut die Liste neu, wenn die Zentrale offen ist.
    pub fn zentrale_auffrischen(&mut self) {
        if self.zentrale.as_ref().is_some_and(|z| z.sichtbar()) {
            self.zentrale_fuellen();
        }
    }

    fn zentrale_fuellen(&self) {
        let Some(zentrale) = &self.zentrale else {
            return;
        };
        let (eintraege, nicht_stoeren) = {
            let z = self.zustand.lock().unwrap_or_else(|e| e.into_inner());
            (
                z.verlauf.eintraege().cloned().collect::<Vec<_>>(),
                z.nicht_stoeren,
            )
        };
        let angezeigt = zentrale.fuellen(&eintraege, nicht_stoeren);
        if std::env::var_os("QB_DEBUG").is_some() {
            dbus::log(&format!(
                "Zentrale: {angezeigt} von {} Einträgen angezeigt",
                eintraege.len()
            ));
        }
    }

    /// Hinweis „in der Zentrale“ im Popup: Zentrale öffnen, Popup zu.
    fn hinweis_geklickt(&mut self, id: u32) {
        self.schliesse(id, grund::GESCHLOSSEN);
        self.zentrale_zeigen("");
    }

    /// Klick auf den Kasten: Default-Aktion, sonst schließen.
    fn klick(&mut self, id: u32) {
        let Some(p) = self.sichtbar.iter().find(|p| p.meldung.id == id) else {
            return;
        };
        if p.meldung.hat_default_aktion() {
            self.aktion(id, "default");
        } else {
            self.weggeklickt(id);
        }
    }
}

fn baue_kasten(m: &Meldung, hinweis: bool) -> gtk4::Box {
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
    // TRANSLATORS: Tooltip des Schließen-Knopfs oben rechts in jedem Popup.
    schliessen.set_tooltip_text(Some(&i18n::gettext("Close")));
    schliessen.set_can_focus(false);
    schliessen.connect_clicked(move |_| spaeter(move |d| d.weggeklickt(id)));
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

    // Fußzeile: Programmname links, Hinweis „in der Zentrale“ rechts (nicht
    // bei transient: die Meldung kommt nicht in den Verlauf).
    let fuss = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    if !m.app_name.is_empty() && !m.summary.is_empty() {
        let app = gtk4::Label::new(Some(&m.app_name));
        app.add_css_class("qb-app");
        app.set_xalign(0.0);
        app.set_width_chars(10);
        app.set_max_width_chars(30);
        app.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        app.set_hexpand(true);
        fuss.append(&app);
    }
    if hinweis && !m.transient {
        // TRANSLATORS: kleiner Hinweis unten rechts im Popup; ein Klick
        // öffnet die Mitteilungszentrale, in der die Meldung gespeichert ist.
        let knopf = gtk4::Button::with_label(&i18n::gettext("in the notification center"));
        knopf.add_css_class("qb-hinweis");
        knopf.set_tooltip_text(Some(&i18n::gettext("Open the notification center")));
        knopf.set_can_focus(false);
        knopf.set_halign(gtk4::Align::End);
        knopf.set_hexpand(true);
        knopf.connect_clicked(move |_| spaeter(move |d| d.hinweis_geklickt(id)));
        fuss.append(&knopf);
    }
    if fuss.first_child().is_some() {
        inhalt.append(&fuss);
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

#[cfg(test)]
mod tests {
    use super::CSS;

    /// Regeln „Selektor { Inhalt }“ eines CSS-Teils ohne Verschachtelung.
    fn regeln(teil: &str) -> Vec<(String, String)> {
        teil.split('}')
            .filter_map(|r| r.split_once('{'))
            .map(|(sel, inhalt)| (sel.trim().to_string(), inhalt.to_string()))
            .collect()
    }

    fn hat_feste_farbe(inhalt: &str) -> bool {
        // Orange (#f07746) ist Akzent und gilt hell wie dunkel.
        inhalt
            .match_indices('#')
            .any(|(i, _)| !inhalt[i..].starts_with("#f07746"))
    }

    #[test]
    fn dunkelblock_deckt_jede_helle_farbe_ab() {
        let (hell, dunkel) = CSS
            .split_once("@media (prefers-color-scheme: dark) {")
            .expect("Dunkelblock fehlt");
        let dunkle_selektoren: Vec<String> = regeln(dunkel)
            .iter()
            .flat_map(|(sel, _)| sel.split(',').map(|s| s.trim().to_string()))
            .collect();
        for (sel, inhalt) in regeln(hell) {
            if hat_feste_farbe(&inhalt) {
                assert!(
                    dunkle_selektoren.contains(&sel),
                    "„{sel}“ hat eine feste Farbe, aber keine dunkle Fassung"
                );
            }
        }
        assert!(!hat_feste_farbe("border: 2px solid #f07746;"));
    }

    /// Erste Farbe `#rrggbb` nach `eigenschaft:` in der Regel `selektor`.
    fn farbe(teil: &str, selektor: &str, eigenschaft: &str) -> [u8; 3] {
        let (_, inhalt) = regeln(teil)
            .into_iter()
            .find(|(sel, _)| sel.split(',').any(|s| s.trim() == selektor))
            .unwrap_or_else(|| panic!("Regel „{selektor}“ fehlt"));
        let wert = inhalt
            .split(';')
            .find_map(|d| d.trim().strip_prefix(eigenschaft)?.strip_prefix(':'))
            .unwrap_or_else(|| panic!("„{selektor}“ ohne {eigenschaft}"));
        let hex = &wert[wert.find('#').expect("keine Farbe") + 1..][..6];
        let k = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap();
        [k(0), k(2), k(4)]
    }

    /// Kontrastverhältnis nach WCAG 2.x (wie kontrast.py des Thema-Repos).
    fn kontrast(a: [u8; 3], b: [u8; 3]) -> f64 {
        let l = |c: [u8; 3]| {
            let k = |v: u8| {
                let v = f64::from(v) / 255.0;
                if v <= 0.03928 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * k(c[0]) + 0.7152 * k(c[1]) + 0.0722 * k(c[2])
        };
        let (h, d) = (l(a).max(l(b)), l(a).min(l(b)));
        (h + 0.05) / (d + 0.05)
    }

    #[test]
    fn programmname_und_hinweis_erreichen_aa() {
        // 0.2.2 (QD-089): hell bis 0.2.1 #7a756f = 4,3:1, jetzt #6f6a64.
        let (hell, dunkel) = CSS
            .split_once("@media (prefers-color-scheme: dark) {")
            .expect("Dunkelblock fehlt");
        for (teil, grund) in [
            (hell, farbe(hell, "box.qb-meldung", "background-color")),
            (dunkel, farbe(dunkel, "box.qb-meldung", "background-color")),
        ] {
            for sel in ["label.qb-app", "button.qb-hinweis"] {
                let k = kontrast(farbe(teil, sel, "color"), grund);
                assert!(k >= 4.5, "„{sel}“ nur {k:.2}:1");
            }
        }
        assert!(kontrast([0x7a, 0x75, 0x6f], [0xf9, 0xf8, 0xf6]) < 4.5);
    }
}
