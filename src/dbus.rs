//! D-Bus-Seite: die Schnittstelle `org.freedesktop.Notifications` (Spez. 1.2)
//! und die eigene `org.querencia.Benachrichtigungen` (Nicht stören, Zustand,
//! Verlauf) am Pfad `/org/freedesktop/Notifications`. Die Methoden laufen auf
//! dem zbus-Ausführer; alles, was Fenster anfasst, wird mit
//! `glib::MainContext::invoke` an den GTK-Thread gereicht.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gtk4::glib;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::OwnedValue;
use zbus::{Connection, interface};

use crate::modell::{Meldung, Zustand, jetzt};
use crate::popups;

pub const NAME: &str = "org.freedesktop.Notifications";
pub const PFAD: &str = "/org/freedesktop/Notifications";
pub const QUERENCIA: &str = "org.querencia.Benachrichtigungen";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Gründe für `NotificationClosed`.
pub mod grund {
    pub const ABGELAUFEN: u32 = 1;
    pub const GESCHLOSSEN: u32 = 2;
    pub const AUFRUF: u32 = 3;
}

pub struct Notifications {
    pub zustand: Arc<Mutex<Zustand>>,
}

#[interface(name = "org.freedesktop.Notifications")]
impl Notifications {
    fn get_capabilities(&self) -> Vec<String> {
        [
            "actions",
            "body",
            "body-markup",
            "body-hyperlinks",
            "icon-static",
            "persistence",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> u32 {
        let (meldung, unterdrueckt) = {
            let mut z = self.zustand.lock().unwrap_or_else(|e| e.into_inner());
            let id = if replaces_id != 0 {
                replaces_id
            } else {
                z.neue_id()
            };
            let meldung = Meldung::neu(
                id,
                app_name,
                app_icon,
                summary,
                body,
                actions,
                &hints,
                expire_timeout,
                jetzt(),
            );
            z.verlauf.merke(&meldung);
            let unterdrueckt = z.nicht_stoeren;
            if !unterdrueckt {
                z.aktiv.insert(id);
            }
            (meldung, unterdrueckt)
        };
        let id = meldung.id;
        let anzahl = self
            .zustand
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .verlauf
            .len() as u32;
        let _ = Querencia::verlauf_geaendert(&emitter, anzahl).await;
        if unterdrueckt {
            log(&format!(
                "Nicht stören: Meldung {id} „{}“ nur in den Verlauf",
                meldung.summary
            ));
            // Der Absender wartet sonst ewig (notify-send --wait).
            let _ = Self::notification_closed(&emitter, id, grund::ABGELAUFEN).await;
        } else {
            glib::MainContext::default().invoke(move || {
                popups::mit_dienst(|d| d.zeige(meldung));
            });
        }
        id
    }

    async fn close_notification(&self, id: u32) -> zbus::fdo::Result<()> {
        let bekannt = self
            .zustand
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .aktiv
            .contains(&id);
        if !bekannt {
            return Err(zbus::fdo::Error::Failed(format!(
                "Meldung {id} gibt es nicht"
            )));
        }
        glib::MainContext::default().invoke(move || {
            popups::mit_dienst(|d| d.schliesse(id, grund::AUFRUF));
        });
        Ok(())
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        (
            "Querencia Benachrichtigungen".into(),
            "Querencia".into(),
            VERSION.into(),
            "1.2".into(),
        )
    }

    #[zbus(signal)]
    pub async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: String,
    ) -> zbus::Result<()>;
}

pub struct Querencia {
    pub zustand: Arc<Mutex<Zustand>>,
}

/// Zustand als Tupel (nicht_stoeren, sichtbar, wartend).
pub fn zustand_tupel(z: &Zustand) -> (bool, u32, u32) {
    (z.nicht_stoeren, z.sichtbar, z.wartend)
}

#[interface(name = "org.querencia.Benachrichtigungen")]
impl Querencia {
    /// Schaltet „Nicht stören“: Popups werden unterdrückt, der Verlauf füllt
    /// sich weiter. Der Zustand überlebt einen Neustart des Dienstes.
    async fn setze_nicht_stoeren(
        &self,
        an: bool,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) {
        let (geaendert, tupel) = {
            let mut z = self.zustand.lock().unwrap_or_else(|e| e.into_inner());
            (z.setze_nicht_stoeren(an), zustand_tupel(&z))
        };
        if geaendert {
            log(&format!("Nicht stören {}", if an { "an" } else { "aus" }));
            let _ = Self::zustand_geaendert(&emitter, tupel.0, tupel.1, tupel.2).await;
        }
    }

    /// (nicht_stoeren, sichtbare Popups, wartende Meldungen)
    fn zustand(&self) -> (bool, u32, u32) {
        zustand_tupel(&self.zustand.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// Die letzten 50 Meldungen, älteste zuerst:
    /// (id, app_name, app_icon, summary, body, urgency, unixzeit).
    fn verlauf(&self) -> Vec<(u32, String, String, String, String, u8, i64)> {
        self.zustand
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .verlauf
            .eintraege()
            .map(|e| e.als_tupel())
            .collect()
    }

    async fn verlauf_loeschen(&self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        self.zustand
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .verlauf
            .leeren();
        let _ = Self::verlauf_geaendert(&emitter, 0).await;
    }

    #[zbus(signal)]
    pub async fn zustand_geaendert(
        emitter: &SignalEmitter<'_>,
        nicht_stoeren: bool,
        sichtbar: u32,
        wartend: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn verlauf_geaendert(emitter: &SignalEmitter<'_>, anzahl: u32) -> zbus::Result<()>;
}

/// Verbindet sich mit dem Sitzungsbus, registriert beide Schnittstellen und
/// belegt den Namen. Ist der Name schon vergeben, kommt ein Fehler mit dem
/// Besitzer zurück und nichts wurde übernommen.
pub fn starten(zustand: Arc<Mutex<Zustand>>) -> Result<Connection, String> {
    zbus::block_on(async {
        let conn = Connection::session()
            .await
            .map_err(|e| format!("kein Sitzungsbus: {e}"))?;
        conn.object_server()
            .at(
                PFAD,
                Notifications {
                    zustand: zustand.clone(),
                },
            )
            .await
            .map_err(|e| format!("Schnittstelle {NAME} nicht registrierbar: {e}"))?;
        conn.object_server()
            .at(PFAD, Querencia { zustand })
            .await
            .map_err(|e| format!("Schnittstelle {QUERENCIA} nicht registrierbar: {e}"))?;
        use zbus::fdo::{RequestNameFlags, RequestNameReply};
        let antwort = conn
            .request_name_with_flags(NAME, RequestNameFlags::DoNotQueue.into())
            .await;
        match antwort {
            Ok(RequestNameReply::PrimaryOwner) => Ok(conn),
            Ok(_) | Err(zbus::Error::NameTaken) => Err(name_belegt(&conn).await),
            Err(e) => Err(format!("Name {NAME} nicht belegbar: {e}")),
        }
    })
}

async fn name_belegt(conn: &Connection) -> String {
    let besitzer = match zbus::fdo::DBusProxy::new(conn).await {
        Ok(p) => match p
            .get_name_owner(zbus::names::BusName::try_from(NAME).expect("gültiger Name"))
            .await
        {
            Ok(b) => {
                let pid = p
                    .get_connection_unix_process_id(zbus::names::BusName::from(b.clone()))
                    .await
                    .map(|n| format!(", PID {n}"))
                    .unwrap_or_default();
                format!("{b}{pid}")
            }
            Err(_) => "unbekannt".into(),
        },
        Err(_) => "unbekannt".into(),
    };
    format!(
        "der Name {NAME} ist bereits belegt (Besitzer {besitzer}); es läuft schon ein Benachrichtigungsdienst, dieser Prozess beendet sich"
    )
}

pub fn log(text: &str) {
    eprintln!("querencia-benachrichtigungen: {text}");
}

// ----------------------------------------------------------------- Client

/// Ruft eine Methode der Querencia-Schnittstelle des laufenden Dienstes.
fn aufruf<B: serde::Serialize + zbus::zvariant::DynamicType>(
    methode: &str,
    body: &B,
) -> Result<zbus::Message, String> {
    zbus::block_on(async {
        let conn = Connection::session()
            .await
            .map_err(|e| format!("kein Sitzungsbus: {e}"))?;
        conn.call_method(Some(NAME), PFAD, Some(QUERENCIA), methode, body)
            .await
            .map_err(|e| match e {
                zbus::Error::MethodError(name, text, _)
                    if name.as_str().ends_with("ServiceUnknown") =>
                {
                    format!(
                        "kein Querencia-Benachrichtigungsdienst erreichbar ({})",
                        text.unwrap_or_default()
                    )
                }
                e => format!("Aufruf {methode} fehlgeschlagen: {e}"),
            })
    })
}

pub fn client_nicht_stoeren(arg: &str) -> Result<String, String> {
    match arg {
        "an" | "aus" => {
            aufruf("SetzeNichtStoeren", &(arg == "an"))?;
            Ok(format!("Nicht stören: {arg}"))
        }
        "status" => {
            let m = aufruf("Zustand", &())?;
            let (ns, sichtbar, wartend): (bool, u32, u32) =
                m.body().deserialize().map_err(|e| e.to_string())?;
            Ok(format!(
                "Nicht stören: {}\nSichtbare Popups: {sichtbar}\nWartende Meldungen: {wartend}",
                if ns { "an" } else { "aus" }
            ))
        }
        _ => Err("--nicht-stoeren erwartet an, aus oder status".into()),
    }
}

pub fn client_verlauf() -> Result<String, String> {
    let m = aufruf("Verlauf", &())?;
    let eintraege: Vec<(u32, String, String, String, String, u8, i64)> =
        m.body().deserialize().map_err(|e| e.to_string())?;
    if eintraege.is_empty() {
        return Ok("Verlauf ist leer".into());
    }
    let mut aus = String::new();
    for (id, app, _symbol, betreff, text, dringlichkeit, zeit) in &eintraege {
        let zeit = glib::DateTime::from_unix_local(*zeit)
            .and_then(|d| d.format("%Y-%m-%d %H:%M:%S"))
            .map(|s| s.to_string())
            .unwrap_or_else(|_| zeit.to_string());
        let d = crate::modell::Dringlichkeit::aus_byte(*dringlichkeit).name();
        let app = if app.is_empty() {
            "(ohne Programm)"
        } else {
            app.as_str()
        };
        aus.push_str(&format!("{zeit}  #{id:<5} [{d:<8}] {app}: {betreff}"));
        if !text.is_empty() {
            aus.push_str(&format!(" – {}", text.replace('\n', " ")));
        }
        aus.push('\n');
    }
    aus.push_str(&format!("{} Einträge", eintraege.len()));
    Ok(aus)
}
