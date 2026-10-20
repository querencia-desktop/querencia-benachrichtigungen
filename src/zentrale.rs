//! Mitteilungszentrale (0.2.0): ein gewöhnliches Fenster (xdg-toplevel, kein
//! Layer-Shell) im Prozess des Dienstes. Schlanke Menüleiste, darunter
//! Suchfeld, Umschalter „Nicht stören“ und „Alle entfernen“, dann die Liste
//! nach Programm gruppiert (Gruppe mit der neuesten Meldung oben, in der
//! Gruppe neueste zuerst), unten eine Statuszeile. Farben und Auswahl kommen
//! aus dem GTK-Thema der Sitzung (Querencia-Klassisch); eigenes CSS nur für
//! Abstände, Fettdruck ungelesener Einträge und den orangen Rand kritischer.
//!
//! Tasten: Esc schließt (im Suchfeld: erst Suche leeren), Entf entfernt den
//! gewählten Eintrag, Eingabe löst die Standardaktion aus (sonst „als
//! gelesen“), Strg+F sucht. Beim Schließen wird das Fenster zerstört und beim
//! nächsten Öffnen neu gebaut: ein nur verstecktes und wieder gezeigtes
//! Fenster verlor in labwc 0.9.8 die Serverdekoration (Titelleiste).
//!
//! Alle Handler gehen über `popups::spaeter` an den Dienst (Wiedereintritt
//! in GTK, siehe Übergabe G2-L); dieses Modul baut nur Widgets.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{gio, glib};

use crate::i18n::{gettext, gettext_f};
use crate::markup;
use crate::modell::Dringlichkeit;
use crate::popups::spaeter;
use crate::symbole;
use crate::verlauf::Eintrag;

/// Kantenlänge der Symbole in der Liste und in den Gruppenköpfen.
const SYMBOL: i32 = 32;
const SYMBOL_KOPF: i32 = 16;
const KOPF_NAME: &str = "qb-z-kopf";

const CSS: &str = r#"
row.qb-z-kopf { padding: 10px 8px 2px 8px; }
label.qb-z-gruppe { font-weight: bold; }
label.qb-z-zahl, label.qb-z-zeit { opacity: 0.75; font-size: 0.9em; }
row.qb-z-eintrag { padding: 6px 8px 6px 10px; }
row.qb-z-kritisch { border-left: 3px solid #f07746; padding-left: 7px; }
label.qb-z-titel.qb-z-ungelesen { font-weight: bold; }
button.qb-z-klein { padding: 1px 8px; min-height: 22px; }
box.qb-z-werkzeug { padding: 6px; }
label.qb-z-status { padding: 3px 8px; }
label.qb-z-leer { opacity: 0.75; }
"#;

pub struct Zentrale {
    pub fenster: gtk4::Window,
    liste: gtk4::ListBox,
    scroll: gtk4::ScrolledWindow,
    stapel: gtk4::Stack,
    leer: gtk4::Label,
    suche: gtk4::SearchEntry,
    ns_knopf: gtk4::CheckButton,
    ns_aktion: gio::SimpleAction,
    status: gtk4::Label,
    /// IDs der Einträge in der angezeigten Reihenfolge.
    ids: RefCell<Vec<u32>>,
    /// Gesetzt, während `fuellen` den Umschalter stellt (kein Rückkanal).
    programmatisch: Rc<Cell<bool>>,
}

fn menue_eintrag(menue: &gio::Menu, text: &str, aktion: &str, kuerzel: Option<&str>) {
    let e = gio::MenuItem::new(Some(text), Some(aktion));
    if let Some(k) = kuerzel {
        e.set_attribute_value("accel", Some(&k.to_variant()));
    }
    menue.append_item(&e);
}

fn kuerzel(controller: &gtk4::ShortcutController, ausloeser: &str, aktion: &str) {
    controller.add_shortcut(gtk4::Shortcut::new(
        gtk4::ShortcutTrigger::parse_string(ausloeser),
        Some(gtk4::NamedAction::new(aktion)),
    ));
}

/// ID eines Eintrags aus dem Widget-Namen der Zeile (`qb-z-<id>`).
fn id_der_zeile(zeile: &gtk4::ListBoxRow) -> Option<u32> {
    zeile.widget_name().strip_prefix("qb-z-")?.parse().ok()
}

static CSS_EINMAL: std::sync::Once = std::sync::Once::new();

impl Zentrale {
    pub fn neu() -> Self {
        CSS_EINMAL.call_once(|| {
            if let Some(display) = gtk4::gdk::Display::default() {
                let css = gtk4::CssProvider::new();
                css.load_from_string(CSS);
                gtk4::style_context_add_provider_for_display(
                    &display,
                    &css,
                    gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
        });
        let fenster = gtk4::Window::builder()
            // TRANSLATORS: Titel des Fensters und Name im Menü (Zubehör).
            .title(gettext("Notification Center"))
            .icon_name("preferences-desktop-notification")
            .default_width(500)
            .default_height(620)
            .build();
        let programmatisch = Rc::new(Cell::new(false));

        // --------------------------------------------------- Aktionen
        let gruppe = gio::SimpleActionGroup::new();
        let a = gio::SimpleAction::new("alle-gelesen", None);
        a.connect_activate(|_, _| spaeter(|d| d.zentrale_als_gelesen(Vec::new())));
        gruppe.add_action(&a);
        let a = gio::SimpleAction::new("alle-entfernen", None);
        let fw = fenster.downgrade();
        a.connect_activate(move |_, _| {
            if let Some(f) = fw.upgrade() {
                alle_entfernen_fragen(&f);
            }
        });
        gruppe.add_action(&a);
        let a = gio::SimpleAction::new("schliessen", None);
        let fw = fenster.downgrade();
        a.connect_activate(move |_, _| {
            if let Some(f) = fw.upgrade() {
                f.close();
            }
        });
        gruppe.add_action(&a);
        let ns_aktion = gio::SimpleAction::new_stateful("nicht-stoeren", None, &false.to_variant());
        let p = programmatisch.clone();
        ns_aktion.connect_change_state(move |aktion, wert| {
            let Some(an) = wert.and_then(|w| w.get::<bool>()) else {
                return;
            };
            aktion.set_state(&an.to_variant());
            if !p.get() {
                spaeter(move |d| d.zentrale_nicht_stoeren(an));
            }
        });
        gruppe.add_action(&ns_aktion);
        fenster.insert_action_group("zentrale", Some(&gruppe));

        // ------------------------------------------------- Menüleiste
        let menue = gio::Menu::new();
        let m = gio::Menu::new();
        menue_eintrag(
            &m,
            &gettext("Mark All as _Read"),
            "zentrale.alle-gelesen",
            None,
        );
        menue_eintrag(&m, &gettext("_Clear All…"), "zentrale.alle-entfernen", None);
        let ende = gio::Menu::new();
        menue_eintrag(
            &ende,
            &gettext("_Close"),
            "zentrale.schliessen",
            Some("Escape"),
        );
        m.append_section(None, &ende);
        menue.append_submenu(Some(&gettext("_Notifications")), &m);
        let m = gio::Menu::new();
        menue_eintrag(&m, &gettext("_Find"), "zentrale.suchen", Some("<Control>f"));
        menue_eintrag(
            &m,
            &gettext("Do Not _Disturb"),
            "zentrale.nicht-stoeren",
            None,
        );
        menue.append_submenu(Some(&gettext("_View")), &m);
        let leiste = gtk4::PopoverMenuBar::from_model(Some(&menue));

        // ------------------------------------------------ Werkzeugzeile
        let suche = gtk4::SearchEntry::new();
        suche.set_placeholder_text(Some(&gettext("Search notifications")));
        suche.set_hexpand(true);
        suche.connect_search_changed(|_| spaeter(|d| d.zentrale_auffrischen()));
        let fw = fenster.downgrade();
        suche.connect_stop_search(move |s| {
            if s.text().is_empty() {
                if let Some(f) = fw.upgrade() {
                    f.close();
                }
            } else {
                s.set_text("");
            }
        });
        let a = gio::SimpleAction::new("suchen", None);
        let sw = suche.downgrade();
        a.connect_activate(move |_, _| {
            if let Some(s) = sw.upgrade() {
                s.grab_focus();
            }
        });
        gruppe.add_action(&a);

        let ns_knopf = gtk4::CheckButton::with_mnemonic(&gettext("Do Not _Disturb"));
        ns_knopf.set_tooltip_text(Some(&gettext(
            "Show only critical notifications as pop-ups; all notifications are still collected here",
        )));
        let p = programmatisch.clone();
        ns_knopf.connect_toggled(move |k| {
            let an = k.is_active();
            if !p.get() {
                spaeter(move |d| d.zentrale_nicht_stoeren(an));
            }
        });
        let alle = gtk4::Button::with_mnemonic(&gettext("C_lear All"));
        alle.set_action_name(Some("zentrale.alle-entfernen"));
        alle.set_tooltip_text(Some(&gettext("Remove all notifications from the list")));

        let werkzeug = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        werkzeug.add_css_class("qb-z-werkzeug");
        werkzeug.append(&suche);
        werkzeug.append(&ns_knopf);
        werkzeug.append(&alle);

        // ------------------------------------------------------- Liste
        let liste = gtk4::ListBox::new();
        liste.add_css_class("qb-z-liste");
        liste.set_selection_mode(gtk4::SelectionMode::Single);
        liste.set_activate_on_single_click(false);
        liste.connect_row_activated(|_, zeile| {
            if let Some(id) = id_der_zeile(zeile) {
                spaeter(move |d| d.zentrale_aktivieren(id));
            }
        });
        let taste = gtk4::EventControllerKey::new();
        let lw = liste.downgrade();
        taste.connect_key_pressed(move |_, key, _, _| {
            if key != gtk4::gdk::Key::Delete && key != gtk4::gdk::Key::KP_Delete {
                return glib::Propagation::Proceed;
            }
            if let Some(id) = lw
                .upgrade()
                .and_then(|l| l.selected_row())
                .and_then(|z| id_der_zeile(&z))
            {
                spaeter(move |d| d.zentrale_entfernen(vec![id]));
            }
            glib::Propagation::Stop
        });
        liste.add_controller(taste);

        let scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .child(&liste)
            .build();
        let leer = gtk4::Label::new(None);
        leer.add_css_class("qb-z-leer");
        leer.set_wrap(true);
        leer.set_justify(gtk4::Justification::Center);
        let stapel = gtk4::Stack::new();
        stapel.add_named(&scroll, Some("liste"));
        stapel.add_named(&leer, Some("leer"));
        stapel.set_vexpand(true);

        let status = gtk4::Label::new(None);
        status.add_css_class("qb-z-status");
        status.set_xalign(0.0);

        let inhalt = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        inhalt.append(&leiste);
        inhalt.append(&werkzeug);
        inhalt.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
        inhalt.append(&stapel);
        inhalt.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
        inhalt.append(&status);
        fenster.set_child(Some(&inhalt));

        let tasten = gtk4::ShortcutController::new();
        kuerzel(&tasten, "Escape", "zentrale.schliessen");
        kuerzel(&tasten, "<Control>w", "zentrale.schliessen");
        kuerzel(&tasten, "<Control>f", "zentrale.suchen");
        fenster.add_controller(tasten);

        fenster.connect_visible_notify(|f| {
            let offen = f.is_visible();
            spaeter(move |d| d.zentrale_sichtbarkeit(offen));
        });
        fenster.connect_close_request(|_| {
            spaeter(|d| d.zentrale_zu());
            glib::Propagation::Proceed
        });

        Self {
            fenster,
            liste,
            scroll,
            stapel,
            leer,
            suche,
            ns_knopf,
            ns_aktion,
            status,
            ids: RefCell::new(Vec::new()),
            programmatisch,
        }
    }

    pub fn sichtbar(&self) -> bool {
        self.fenster.is_visible()
    }

    /// Zeigt das Fenster; `token` ist ein xdg-activation-Token oder leer.
    pub fn zeigen(&self, token: &str) {
        if !token.is_empty() {
            self.fenster.set_startup_id(token);
        }
        self.fenster.present();
        if self.liste.selected_row().is_none()
            && let Some(id) = self.ids.borrow().first()
        {
            self.waehle(*id, true);
        }
    }

    /// Zeile mit Eintrag `id` wählen und (auf Wunsch) fokussieren.
    fn waehle(&self, id: u32, fokus: bool) {
        let name = format!("qb-z-{id}");
        let mut kind = self.liste.first_child();
        while let Some(w) = kind {
            if let Ok(zeile) = w.clone().downcast::<gtk4::ListBoxRow>()
                && zeile.widget_name() == name
            {
                self.liste.select_row(Some(&zeile));
                if fokus {
                    zeile.grab_focus();
                }
                return;
            }
            kind = w.next_sibling();
        }
    }

    /// Baut die Liste neu (Suchtext, Auswahl und Bildlauf bleiben); liefert
    /// die Zahl der angezeigten Einträge.
    pub fn fuellen(&self, eintraege: &[Eintrag], nicht_stoeren: bool) -> usize {
        self.programmatisch.set(true);
        self.ns_knopf.set_active(nicht_stoeren);
        self.ns_aktion.set_state(&nicht_stoeren.to_variant());
        self.programmatisch.set(false);

        let alt_ids = self.ids.borrow().clone();
        let gewaehlt = self.liste.selected_row().and_then(|z| id_der_zeile(&z));
        let alt_index = gewaehlt.and_then(|id| alt_ids.iter().position(|&i| i == id));
        let fokus = gtk4::prelude::GtkWindowExt::focus(&self.fenster)
            .is_some_and(|w| w.is_ancestor(&self.liste));
        let bildlauf = self.scroll.vadjustment().value();

        self.liste.remove_all();
        let suche = self.suche.text();
        let mut gruppen: Vec<(String, Vec<&Eintrag>)> = Vec::new();
        for e in eintraege.iter().filter(|e| e.passt(&suche)) {
            match gruppen.iter_mut().find(|(g, _)| g == e.gruppe()) {
                Some((_, v)) => v.push(e),
                None => gruppen.push((e.gruppe().to_string(), vec![e])),
            }
        }
        for (_, v) in gruppen.iter_mut() {
            v.sort_by_key(|e| std::cmp::Reverse((e.zeit, e.id)));
        }
        gruppen.sort_by_key(|(_, v)| std::cmp::Reverse((v[0].zeit, v[0].id)));

        let jetzt = glib::DateTime::now_local().ok();
        let mut ids = Vec::new();
        for (name, v) in &gruppen {
            self.liste.append(&kopfzeile(name, v));
            for e in v {
                self.liste.append(&zeile(e, jetzt.as_ref()));
                ids.push(e.id);
            }
        }

        let ungelesen = eintraege.iter().filter(|e| !e.gelesen).count();
        self.status.set_text(&gettext_f(
            "Notifications: {anzahl} · Unread: {ungelesen}",
            &[
                ("anzahl", &eintraege.len().to_string()),
                ("ungelesen", &ungelesen.to_string()),
            ],
        ));
        if eintraege.is_empty() {
            self.leer.set_text(&gettext("No notifications"));
            self.stapel.set_visible_child_name("leer");
        } else if ids.is_empty() {
            self.leer
                .set_text(&gettext("No notifications match the search"));
            self.stapel.set_visible_child_name("leer");
        } else {
            self.stapel.set_visible_child_name("liste");
        }

        let neu_auswahl = match (gewaehlt, alt_index) {
            (Some(id), _) if ids.contains(&id) => Some(id),
            (_, Some(i)) if !ids.is_empty() => Some(ids[i.min(ids.len() - 1)]),
            _ => None,
        };
        let angezeigt = ids.len();
        *self.ids.borrow_mut() = ids;
        if let Some(id) = neu_auswahl {
            self.waehle(id, fokus);
        }
        let adj = self.scroll.vadjustment();
        glib::idle_add_local_once(move || adj.set_value(bildlauf));
        angezeigt
    }
}

/// Fragt vor „Alle entfernen“ nach (der Verlauf ist danach leer).
fn alle_entfernen_fragen(fenster: &gtk4::Window) {
    let abbrechen = gettext("_Cancel");
    let entfernen = gettext("_Remove All");
    let dialog = gtk4::AlertDialog::builder()
        .modal(true)
        .message(gettext("Remove all notifications?"))
        .detail(gettext("The list will be empty. This cannot be undone."))
        .buttons([abbrechen.as_str(), entfernen.as_str()])
        .cancel_button(0)
        .default_button(1)
        .build();
    dialog.choose(Some(fenster), gio::Cancellable::NONE, |antwort| {
        if antwort == Ok(1) {
            spaeter(|d| d.zentrale_alle_entfernen());
        }
    });
}

fn kopfzeile(name: &str, v: &[&Eintrag]) -> gtk4::ListBoxRow {
    let zeile = gtk4::ListBoxRow::new();
    zeile.set_widget_name(KOPF_NAME);
    zeile.add_css_class(KOPF_NAME);
    zeile.set_selectable(false);
    zeile.set_activatable(false);
    zeile.set_focusable(false);
    let kasten = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    kasten.append(&symbole::widget_eintrag(v[0], SYMBOL_KOPF));
    let titel = if name.is_empty() {
        gettext("Other")
    } else {
        name.to_string()
    };
    let l = gtk4::Label::new(Some(&titel));
    l.add_css_class("qb-z-gruppe");
    l.set_xalign(0.0);
    l.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    l.set_hexpand(true);
    kasten.append(&l);
    let ungelesen = v.iter().filter(|e| !e.gelesen).count();
    let zahl = if ungelesen > 0 {
        gettext_f(
            "{anzahl} ({ungelesen} unread)",
            &[
                ("anzahl", &v.len().to_string()),
                ("ungelesen", &ungelesen.to_string()),
            ],
        )
    } else {
        v.len().to_string()
    };
    let z = gtk4::Label::new(Some(&zahl));
    z.add_css_class("qb-z-zahl");
    kasten.append(&z);
    zeile.set_child(Some(&kasten));
    zeile
}

fn zeile(e: &Eintrag, jetzt: Option<&glib::DateTime>) -> gtk4::ListBoxRow {
    let id = e.id;
    let zeile = gtk4::ListBoxRow::new();
    zeile.set_widget_name(&format!("qb-z-{id}"));
    zeile.add_css_class("qb-z-eintrag");
    if e.dringlichkeit() == Dringlichkeit::Kritisch {
        zeile.add_css_class("qb-z-kritisch");
    }
    let kasten = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    let symbol = symbole::widget_eintrag(e, SYMBOL);
    symbol.set_valign(gtk4::Align::Start);
    kasten.append(&symbol);

    let spalte = gtk4::Box::new(gtk4::Orientation::Vertical, 3);
    spalte.set_hexpand(true);
    let kopf = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let titel = gtk4::Label::new(Some(if e.summary.is_empty() {
        &e.app_name
    } else {
        &e.summary
    }));
    titel.add_css_class("qb-z-titel");
    if !e.gelesen {
        titel.add_css_class("qb-z-ungelesen");
    }
    titel.set_xalign(0.0);
    titel.set_hexpand(true);
    titel.set_wrap(true);
    titel.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    titel.set_width_chars(10);
    titel.set_max_width_chars(40);
    titel.set_lines(2);
    titel.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    kopf.append(&titel);
    let zeit = gtk4::Label::new(Some(
        &jetzt.map(|j| zeit_text(e.zeit, j)).unwrap_or_default(),
    ));
    zeit.add_css_class("qb-z-zeit");
    zeit.set_valign(gtk4::Align::Start);
    kopf.append(&zeit);
    spalte.append(&kopf);

    if !e.body.is_empty() {
        let text = gtk4::Label::new(None);
        text.add_css_class("qb-z-text");
        text.set_xalign(0.0);
        text.set_wrap(true);
        text.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
        text.set_width_chars(10);
        text.set_max_width_chars(50);
        text.set_lines(3);
        text.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        let markup = markup::bereinige(&e.body);
        if gtk4::pango::parse_markup(&markup::pango_pruefbar(&markup), '\0').is_ok() {
            text.set_markup(&markup);
        } else {
            text.set_text(&e.body);
        }
        spalte.append(&text);
    }

    let knoepfe = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    knoepfe.set_margin_top(2);
    // Aktionen nur aus dem laufenden Dienst: nach einem Neustart hört der
    // Absender nicht mehr zu (seine Verbindung ist weg).
    if e.aus_diesem_lauf {
        for (schluessel, beschriftung) in e.aktionen.iter().filter(|(k, _)| k != "default") {
            let k = gtk4::Button::with_label(beschriftung);
            k.add_css_class("qb-z-klein");
            let schluessel = schluessel.clone();
            k.connect_clicked(move |_| {
                let schluessel = schluessel.clone();
                spaeter(move |d| d.zentrale_aktion(id, &schluessel));
            });
            knoepfe.append(&k);
        }
    }
    let luecke = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    luecke.set_hexpand(true);
    knoepfe.append(&luecke);
    if !e.gelesen {
        let k = gtk4::Button::with_label(&gettext("Mark as Read"));
        k.add_css_class("qb-z-klein");
        k.connect_clicked(move |_| spaeter(move |d| d.zentrale_als_gelesen(vec![id])));
        knoepfe.append(&k);
    }
    let k = gtk4::Button::with_label(&gettext("Remove"));
    k.add_css_class("qb-z-klein");
    k.set_tooltip_text(Some(&gettext("Remove this notification (Delete)")));
    k.connect_clicked(move |_| spaeter(move |d| d.zentrale_entfernen(vec![id])));
    knoepfe.append(&k);
    spalte.append(&knoepfe);

    kasten.append(&spalte);
    zeile.set_child(Some(&kasten));
    zeile
}

/// Zeit für die Liste: heute „14:05“, gestern „Gestern, 14:05“, sonst mit
/// Datum (Ziffern, damit die Sitzung mit `LANG=en_US` und `LANGUAGE=de` keine
/// englischen Monatsnamen zeigt).
pub fn zeit_text(zeit: i64, jetzt: &glib::DateTime) -> String {
    let Ok(d) = glib::DateTime::from_unix_local(zeit) else {
        return String::new();
    };
    let tag = |x: &glib::DateTime| (x.year(), x.month(), x.day_of_month());
    let gestern = jetzt.add_days(-1).ok();
    let format = if tag(&d) == tag(jetzt) {
        "%H:%M".to_string()
    } else if gestern.as_ref().is_some_and(|g| tag(g) == tag(&d)) {
        // TRANSLATORS: Zeit einer Mitteilung von gestern (strftime-Format).
        gettext("Yesterday, %H:%M")
    } else if d.year() == jetzt.year() {
        // TRANSLATORS: Datum und Zeit einer älteren Mitteilung im laufenden
        // Jahr (strftime-Format, nur Ziffern).
        gettext("%m/%d, %H:%M")
    } else {
        // TRANSLATORS: Datum und Zeit einer Mitteilung aus einem anderen Jahr.
        gettext("%m/%d/%Y, %H:%M")
    };
    d.format(&format).map(|s| s.to_string()).unwrap_or_default()
}
