//! Querencia Benachrichtigungen: Dienst für Desktop-Benachrichtigungen nach der
//! Freedesktop-Spezifikation (org.freedesktop.Notifications) mit klassischen
//! Popups (gtk4-layer-shell), Mitteilungszentrale mit Verlauf und „Nicht
//! stören“ über org.querencia.Benachrichtigungen (docs/dbus.md).
//!
//!   querencia-benachrichtigungen                     Dienst starten
//!   querencia-benachrichtigungen --zentrale          Mitteilungszentrale zeigen
//!   querencia-benachrichtigungen --nicht-stoeren an|aus|status
//!   querencia-benachrichtigungen --verlauf
//!   querencia-benachrichtigungen --version | --hilfe

mod dbus;
mod einstellung;
mod i18n;
mod markup;
mod modell;
mod popups;
mod symbole;
mod verlauf;
mod zentrale;

use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use gtk4::prelude::*;
use gtk4::{gdk, glib};

/// Programmname = Desktop-ID des Starters: daran ordnen labwc und die
/// Fensterliste das Fenster der Zentrale dem Starter zu (xdg-toplevel app_id).
const PROGRAMMNAME: &str = "org.querencia.Benachrichtigungen";

const HILFE: &str = "Aufruf: querencia-benachrichtigungen [OPTION]
  (ohne Option)                 Benachrichtigungsdienst starten
  --zentrale                    Mitteilungszentrale zeigen (startet den Dienst,
                                wenn keiner läuft)
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
        [] => return dienst(false),
        ["--zentrale"] => return zentrale(),
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

/// `--zentrale`: den laufenden Dienst bitten, das Fenster zu zeigen. Läuft
/// kein Benachrichtigungsdienst, wird dieser Prozess der Dienst und zeigt die
/// Zentrale selbst. Hält ein fremder oder älterer Dienst den Namen, erklärt
/// ein kleines Fenster, warum es keine Zentrale gibt.
fn zentrale() -> ExitCode {
    let token = std::env::var("XDG_ACTIVATION_TOKEN").unwrap_or_default();
    match dbus::client_zentrale(&token) {
        Ok(dbus::ZentraleAufruf::Gezeigt) => ExitCode::SUCCESS,
        Ok(dbus::ZentraleAufruf::KeinDienst) => {
            dbus::log("kein Benachrichtigungsdienst erreichbar; starte ihn mit der Zentrale");
            dienst(true)
        }
        Ok(dbus::ZentraleAufruf::Fremd(name, version)) => {
            dbus::log(&format!(
                "{} gehört „{name}“ {version} ohne Mitteilungszentrale",
                dbus::NAME
            ));
            hinweisfenster(&name, &version);
            ExitCode::FAILURE
        }
        Err(text) => {
            eprintln!("querencia-benachrichtigungen: {text}");
            ExitCode::FAILURE
        }
    }
}

/// Kleines Fenster für `--zentrale`, wenn ein anderer Dienst läuft (z. B.
/// noch 0.1.x bis zur nächsten Anmeldung).
fn hinweisfenster(name: &str, version: &str) {
    glib::set_prgname(Some(PROGRAMMNAME));
    if gtk4::init().is_err() {
        return;
    }
    let text = if name == "Querencia Benachrichtigungen" {
        i18n::gettext(
            "The notification center will be available after the next login. The running notification service is an older version.",
        )
    } else {
        i18n::gettext_f(
            "Another notification service is running ({name} {version}). The notification center is not available.",
            &[("name", name), ("version", version)],
        )
    };
    let schleife = glib::MainLoop::new(None, false);
    let dialog = gtk4::AlertDialog::builder()
        .message(i18n::gettext("Notification Center"))
        .detail(text)
        .buttons([i18n::gettext("_Close")])
        .default_button(0)
        .cancel_button(0)
        .build();
    let s = schleife.clone();
    dialog.choose(
        gtk4::Window::NONE,
        gtk4::gio::Cancellable::NONE,
        move |_| s.quit(),
    );
    schleife.run();
}

fn dienst(zentrale_zeigen: bool) -> ExitCode {
    glib::set_prgname(Some(PROGRAMMNAME));
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
    let state = zustandsverzeichnis().join("querencia");
    let config = std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from);
    let (zustand, log) = modell::Zustand::neu(modell::Orte {
        nicht_stoeren: Some(state.join("benachrichtigungen-nicht-stoeren")),
        verlauf: Some(state.join("benachrichtigungen.json")),
        ini: Some(einstellung::ini_pfad(config.as_deref(), &glib::home_dir())),
    });
    for zeile in log {
        dbus::log(&zeile);
    }
    if zustand.nicht_stoeren {
        dbus::log("Nicht stören ist an (gespeicherter Zustand)");
    }
    let zustand = Arc::new(Mutex::new(zustand));
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
    if zentrale_zeigen {
        let token = std::env::var("XDG_ACTIVATION_TOKEN").unwrap_or_default();
        glib::idle_add_local_once(move || {
            popups::mit_dienst(|d| d.zentrale_zeigen(&token));
        });
    }

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
