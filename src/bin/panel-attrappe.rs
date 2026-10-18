//! Panel-Attrappe für die isolierte Prüfung: ein oberes Panel als Layer-Shell-
//! Fläche (Ebene top, links/rechts/oben verankert) mit automatischer
//! Exklusivzone, wie das Querencia Panel. Aufruf: panel-attrappe [HÖHE]
//! (Standard 32). Nicht Teil des Dienstes.

use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

fn main() {
    let hoehe: i32 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(32);
    gtk4::init().expect("GTK");
    let css = gtk4::CssProvider::new();
    css.load_from_string("window.attrappe { background-color: #d6d2ce; border-bottom: 1px solid #8f8a85; } label { color: #2e2e2e; }");
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("Display"),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    let label = gtk4::Label::new(Some(&format!("Panel-Attrappe (Exklusivzone {hoehe} px)")));
    let fenster = gtk4::Window::builder()
        .title("Panel-Attrappe")
        .child(&label)
        .default_height(hoehe)
        .build();
    fenster.add_css_class("attrappe");
    fenster.set_size_request(-1, hoehe);
    fenster.init_layer_shell();
    fenster.set_namespace(Some("panel-attrappe"));
    fenster.set_layer(Layer::Top);
    fenster.set_keyboard_mode(KeyboardMode::None);
    for e in [Edge::Top, Edge::Left, Edge::Right] {
        fenster.set_anchor(e, true);
    }
    fenster.auto_exclusive_zone_enable();
    fenster.present();
    let schleife = gtk4::glib::MainLoop::new(None, false);
    schleife.run();
}
