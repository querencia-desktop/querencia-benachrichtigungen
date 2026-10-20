//! Querencia Benachrichtigungen: Dienst für Desktop-Benachrichtigungen nach der
//! Freedesktop-Spezifikation (org.freedesktop.Notifications) mit klassischen
//! Popups oben rechts (gtk4-layer-shell) und „Nicht stören“ über
//! org.querencia.Benachrichtigungen.
//!
//!   querencia-benachrichtigungen                     Dienst starten
//!   querencia-benachrichtigungen --nicht-stoeren an|aus|status
//!   querencia-benachrichtigungen --verlauf
//!   querencia-benachrichtigungen --version | --hilfe

mod dbus;
mod i18n;
mod markup;
mod modell;
mod popups;
mod symbole;

use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use gtk4::prelude::*;
use gtk4::{gdk, glib};

const HILFE: &str = "Aufruf: querencia-benachrichtigungen [OPTION]
  (ohne Option)                 Benachrichtigungsdienst starten
  --nicht-stoeren an|aus|status „Nicht stören“ schalten oder abfragen
  --verlauf                     die letzten 50 Meldungen ausgeben
  --version                     Version ausgeben
  --hilfe, -h                   diese Hilfe";

fn main() -> ExitCode {
    i18n::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ergebnis = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => return dienst(),
        ["--nicht-stoeren", wert] => dbus::client_nicht_stoeren(wert),
        ["--verlauf"] => dbus::client_verlauf(),
        ["--version"] => Ok(format!("querencia-benachrichtigungen {}", dbus::VERSION)),
        ["--hilfe"] | ["-h"] | ["--help"] => Ok(HILFE.to_string()),
        _ => Err(format!("unbekannte Argumente: {}\n{HILFE}", args.join(" "))),
    };
    match ergebnis {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(text) => {
            eprintln!("querencia-benachrichtigungen: {text}");
            ExitCode::FAILURE
        }
    }
}

fn dienst() -> ExitCode {
    glib::set_prgname(Some("querencia-benachrichtigungen"));
    // Nur Wayland (QD-062): Ohne erreichbaren Compositor fiel GTK am
    // 20.10.2026 auf X11 (Xwayland) zurück; dort gibt es kein Layer-Shell.
    // Der Dienst beendet sich dann, bevor er den Busnamen belegt.
    gdk::set_allowed_backends("wayland");
    if let Err(e) = gtk4::init() {
        dbus::log(&format!(
            "kein Wayland-Compositor erreichbar ({e}); beende, ohne {} zu belegen",
            dbus::NAME
        ));
        return ExitCode::FAILURE;
    }
    let Some(display) = gdk::Display::default().filter(ist_wayland) else {
        dbus::log(&format!(
            "keine Wayland-Verbindung; beende, ohne {} zu belegen",
            dbus::NAME
        ));
        return ExitCode::FAILURE;
    };
    if !gtk4_layer_shell::is_supported() {
        dbus::log(&format!(
            "der Compositor bietet kein wlr-layer-shell; keine Popups möglich, beende, ohne {} zu belegen",
            dbus::NAME
        ));
        return ExitCode::FAILURE;
    }
    // Verlorene Verbindung: sofort beenden, damit der Busname frei wird.
    // (GDK beendet den Prozess bei einem Lesefehler meist selbst; das hier
    // gilt für jedes Schließen des Displays.)
    display.connect_closed(|_, fehler| {
        dbus::log(&format!(
            "Wayland-Verbindung {}; beende sofort und gebe {} frei",
            if fehler { "verloren" } else { "geschlossen" },
            dbus::NAME
        ));
        std::process::exit(1);
    });
    let zustandsdatei = zustandsverzeichnis()
        .join("querencia")
        .join("benachrichtigungen-nicht-stoeren");
    let zustand = Arc::new(Mutex::new(modell::Zustand::neu(Some(zustandsdatei))));
    if zustand.lock().map(|z| z.nicht_stoeren).unwrap_or(false) {
        dbus::log("Nicht stören ist an (gespeicherter Zustand)");
    }
    let conn = match dbus::starten(zustand.clone()) {
        Ok(c) => c,
        Err(e) => {
            dbus::log(&e);
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = popups::einrichten(conn, zustand) {
        dbus::log(&e);
        return ExitCode::FAILURE;
    }
    dbus::log(&format!(
        "bereit: {} auf dem Sitzungsbus, Version {}",
        dbus::NAME,
        dbus::VERSION
    ));

    let schleife = glib::MainLoop::new(None, false);
    let s = schleife.clone();
    glib_unix::unix_signal_add_local(libc_sigterm(), move || {
        dbus::log("SIGTERM, beende");
        s.quit();
        glib::ControlFlow::Break
    });
    let s = schleife.clone();
    glib_unix::unix_signal_add_local(libc_sigint(), move || {
        dbus::log("SIGINT, beende");
        s.quit();
        glib::ControlFlow::Break
    });
    schleife.run();
    ExitCode::SUCCESS
}

/// Ob `display` eine Wayland-Verbindung ist (nicht X11).
fn ist_wayland(display: &gdk::Display) -> bool {
    ist_wayland_typ(display.type_().name())
}

/// Typname der Wayland-Displays von GDK 4.
fn ist_wayland_typ(name: &str) -> bool {
    name == "GdkWaylandDisplay"
}

/// `$XDG_STATE_HOME`, sonst `~/.local/state` (wie der Sitzungsstarter).
fn zustandsverzeichnis() -> std::path::PathBuf {
    match std::env::var_os("XDG_STATE_HOME") {
        Some(p) if !p.is_empty() && std::path::Path::new(&p).is_absolute() => {
            std::path::PathBuf::from(p)
        }
        _ => glib::home_dir().join(".local").join("state"),
    }
}

// Signalnummern ohne libc-Crate (Linux).
fn libc_sigterm() -> i32 {
    15
}
fn libc_sigint() -> i32 {
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nur_wayland_displays_zaehlen() {
        assert!(ist_wayland_typ("GdkWaylandDisplay"));
        assert!(!ist_wayland_typ("GdkX11Display"));
        assert!(!ist_wayland_typ("GdkBroadwayDisplay"));
        assert!(!ist_wayland_typ(""));
    }
}
