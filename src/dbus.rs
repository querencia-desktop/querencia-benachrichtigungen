//! D-Bus-Seite: die Schnittstelle `org.freedesktop.Notifications` (Spez. 1.2)
//! und die eigene `org.querencia.Benachrichtigungen` (Verlauf der
//! Mitteilungszentrale, gelesen/ungelesen, Nicht stören, Zentrale zeigen) am
//! Pfad `/org/freedesktop/Notifications`; beschrieben in `docs/dbus.md`. Der
//! Dienst belegt beide Busnamen. Die Methoden laufen auf dem zbus-Ausführer;
//! alles, was Fenster anfasst, wird mit `glib::MainContext::invoke` an den
//! GTK-Thread gereicht.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gtk4::glib;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, interface};

use crate::modell::{self, Meldung, Zustand, jetzt};
use crate::popups;
use crate::verlauf::{ALT_ANZAHL, EintragTupel, Gemerkt};

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

fn sperre(z: &Mutex<Zustand>) -> std::sync::MutexGuard<'_, Zustand> {
    z.lock().unwrap_or_else(|e| e.into_inner())
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
        #[zbus(connection)] conn: &Connection,
    ) -> u32 {
        let (meldung, zeigen, gemerkt, tupel, einstellungs_log) = {
            let mut z = sperre(&self.zustand);
            let einstellungs_log = z.einstellungen_auffrischen();
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
            let gemerkt = z.verlauf.merke(&meldung);
            if gemerkt != Gemerkt::Nein {
                z.verlauf_sichern();
            }
            let zeigen = modell::einblenden(meldung.dringlichkeit, z.nicht_stoeren, z.ein());
            if zeigen {
                z.aktiv.insert(id);
            }
            let tupel = z.verlauf.eintrag(id).map(|e| e.als_tupel());
            (meldung, zeigen, gemerkt, tupel, einstellungs_log)
        };
        for zeile in einstellungs_log {
            log(&zeile);
        }
        let id = meldung.id;
        match (gemerkt, tupel) {
            (Gemerkt::Neu, Some(t)) => melde_neu(conn, t).await,
            (Gemerkt::Ersetzt, _) => melde_geaendert(conn, vec![id]).await,
            _ => {}
        }
        if gemerkt != Gemerkt::Nein {
            melde_zaehler(conn, &self.zustand).await;
        }
        if !zeigen {
            let wegen = {
                let z = sperre(&self.zustand);
                if z.nicht_stoeren {
                    "Nicht stören"
                } else {
                    "Einblendungen aus"
                }
            };
            log(&format!(
                "{wegen}: Meldung {id} „{}“ nur in den Verlauf",
                meldung.summary
            ));
        }
        glib::MainContext::default().invoke(move || {
            popups::mit_dienst(|d| {
                if zeigen {
                    d.zeige(meldung);
                } else {
                    // Der Absender wartet sonst ewig (notify-send --wait).
                    d.unterdrueckt(id);
                }
                if gemerkt != Gemerkt::Nein {
                    d.zentrale_auffrischen();
                }
            });
        });
        id
    }

    async fn close_notification(&self, id: u32) -> zbus::fdo::Result<()> {
        let bekannt = sperre(&self.zustand).aktiv.contains(&id);
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
    // ------------------------------------------------ Mitteilungszentrale (0.2.0)

    /// Alle Einträge des Verlaufs, neueste zuerst.
    fn liste(&self) -> Vec<EintragTupel> {
        sperre(&self.zustand)
            .verlauf
            .eintraege()
            .rev()
            .map(|e| e.als_tupel())
            .collect()
    }

    /// Markiert die Einträge `ids` als gelesen; ein leeres Feld heißt alle.
    async fn als_gelesen(&self, ids: Vec<u32>, #[zbus(connection)] conn: &Connection) {
        let geaendert = {
            let mut z = sperre(&self.zustand);
            let g = z.verlauf.als_gelesen(&ids);
            if !g.is_empty() {
                z.verlauf_sichern();
            }
            g
        };
        if !geaendert.is_empty() {
            nach_aenderung(conn, &self.zustand, geaendert, Vec::new()).await;
        }
    }

    /// Entfernt die Einträge `ids` aus dem Verlauf; sichtbare Popups dazu
    /// schließen sich (Grund 2).
    async fn entfernen(&self, ids: Vec<u32>, #[zbus(connection)] conn: &Connection) {
        let weg = {
            let mut z = sperre(&self.zustand);
            let w = z.verlauf.entfernen(&ids);
            if !w.is_empty() {
                z.verlauf_sichern();
            }
            w
        };
        if !weg.is_empty() {
            nach_aenderung(conn, &self.zustand, weg.clone(), weg).await;
        }
    }

    /// Leert den Verlauf und schließt alle Popups (Grund 2).
    async fn alle_entfernen(&self, #[zbus(connection)] conn: &Connection) {
        let weg: Vec<u32> = {
            let mut z = sperre(&self.zustand);
            let ids = z.verlauf.eintraege().map(|e| e.id).collect();
            z.verlauf.leeren();
            z.verlauf_sichern();
            ids
        };
        nach_aenderung(conn, &self.zustand, Vec::new(), weg).await;
    }

    /// Zeigt das Fenster der Mitteilungszentrale (oder holt es nach vorn).
    /// `aktivierungs_token`: xdg-activation-Token des Aufrufers oder leer.
    fn zentrale_zeigen(&self, aktivierungs_token: String) {
        glib::MainContext::default().invoke(move || {
            popups::mit_dienst(|d| d.zentrale_zeigen(&aktivierungs_token));
        });
    }

    /// Schließt das Fenster der Mitteilungszentrale, falls es offen ist.
    fn zentrale_schliessen(&self) {
        glib::MainContext::default().invoke(move || {
            popups::mit_dienst(|d| d.zentrale_schliessen());
        });
    }

    /// „Nicht stören“: unterdrückt Popups außer kritischen; der Verlauf
    /// füllt sich weiter. Überlebt einen Neustart des Dienstes.
    #[zbus(property)]
    fn nicht_stoeren(&self) -> bool {
        sperre(&self.zustand).nicht_stoeren
    }

    #[zbus(property)]
    async fn set_nicht_stoeren(&self, an: bool) {
        nicht_stoeren_setzen(&self.zustand, an, None).await;
    }

    /// Zahl der ungelesenen Einträge.
    #[zbus(property(emits_changed_signal = "true"))]
    fn ungelesen(&self) -> u32 {
        sperre(&self.zustand).verlauf.ungelesen() as u32
    }

    /// Zahl aller Einträge.
    #[zbus(property(emits_changed_signal = "true"))]
    fn anzahl(&self) -> u32 {
        sperre(&self.zustand).verlauf.len() as u32
    }

    /// Ob das Fenster der Zentrale offen ist.
    #[zbus(property(emits_changed_signal = "true"))]
    fn zentrale_offen(&self) -> bool {
        sperre(&self.zustand).zentrale_offen
    }

    /// Ein neuer Eintrag im Verlauf (nicht bei `transient`).
    #[zbus(signal)]
    pub async fn neu(emitter: &SignalEmitter<'_>, eintrag: EintragTupel) -> zbus::Result<()>;

    /// Einträge haben sich geändert (gelesen, ersetzt) oder sind weg; neu
    /// mit `Liste` lesen. Leeres Feld: alles kann sich geändert haben.
    #[zbus(signal)]
    pub async fn geaendert(emitter: &SignalEmitter<'_>, ids: Vec<u32>) -> zbus::Result<()>;

    /// Neuer Stand der Zähler (nach jeder Änderung des Verlaufs).
    #[zbus(signal)]
    pub async fn zaehler(
        emitter: &SignalEmitter<'_>,
        ungelesen: u32,
        anzahl: u32,
    ) -> zbus::Result<()>;

    // -------------------------------------------------- bis 0.1.3 (bleibt)

    /// Schaltet „Nicht stören“ (wie die Eigenschaft `NichtStoeren`).
    async fn setze_nicht_stoeren(&self, an: bool, #[zbus(connection)] conn: &Connection) {
        nicht_stoeren_setzen(&self.zustand, an, Some(conn)).await;
    }

    /// (nicht_stoeren, sichtbare Popups, wartende Meldungen)
    fn zustand(&self) -> (bool, u32, u32) {
        zustand_tupel(&sperre(&self.zustand))
    }

    /// Die letzten 50 Meldungen, älteste zuerst:
    /// (id, app_name, app_icon, summary, body, urgency, unixzeit).
    fn verlauf(&self) -> Vec<(u32, String, String, String, String, u8, i64)> {
        sperre(&self.zustand)
            .verlauf
            .letzte(ALT_ANZAHL)
            .map(|e| e.als_alt_tupel())
            .collect()
    }

    /// Wie `AlleEntfernen`.
    async fn verlauf_loeschen(&self, #[zbus(connection)] conn: &Connection) {
        self.alle_entfernen(conn).await;
    }

    #[zbus(signal)]
    pub async fn zustand_geaendert(
        emitter: &SignalEmitter<'_>,
        nicht_stoeren: bool,
        sichtbar: u32,
        wartend: u32,
    ) -> zbus::Result<()>;

    /// Anzahl der Einträge nach jeder Änderung des Verlaufs.
    #[zbus(signal)]
    pub async fn verlauf_geaendert(emitter: &SignalEmitter<'_>, anzahl: u32) -> zbus::Result<()>;
}

fn emitter(conn: &Connection) -> Option<SignalEmitter<'static>> {
    SignalEmitter::new(conn, PFAD).ok()
}

/// Sendet `PropertiesChanged` für die Querencia-Schnittstelle.
async fn eigenschaften_geaendert(conn: &Connection, werte: &[(&str, Value<'_>)]) {
    let Some(e) = emitter(conn) else { return };
    let geaendert: HashMap<&str, Value<'_>> = werte
        .iter()
        .map(|(k, v)| (*k, v.try_clone().unwrap_or(Value::U32(0))))
        .collect();
    let iface = zbus::names::InterfaceName::from_static_str_unchecked(QUERENCIA);
    if let Err(err) =
        zbus::fdo::Properties::properties_changed(&e, iface, geaendert, Default::default()).await
    {
        log(&format!("PropertiesChanged nicht gesendet: {err}"));
    }
}

pub async fn melde_neu(conn: &Connection, t: EintragTupel) {
    if let Some(e) = emitter(conn) {
        let _ = Querencia::neu(&e, t).await;
    }
}

pub async fn melde_geaendert(conn: &Connection, ids: Vec<u32>) {
    if let Some(e) = emitter(conn) {
        let _ = Querencia::geaendert(&e, ids).await;
    }
}

/// `Zaehler`, `PropertiesChanged` (Ungelesen, Anzahl) und das alte
/// `VerlaufGeaendert`.
pub async fn melde_zaehler(conn: &Connection, zustand: &Mutex<Zustand>) {
    let (ungelesen, anzahl) = {
        let z = sperre(zustand);
        (z.verlauf.ungelesen() as u32, z.verlauf.len() as u32)
    };
    if let Some(e) = emitter(conn) {
        let _ = Querencia::zaehler(&e, ungelesen, anzahl).await;
        let _ = Querencia::verlauf_geaendert(&e, anzahl).await;
    }
    eigenschaften_geaendert(
        conn,
        &[
            ("Ungelesen", Value::U32(ungelesen)),
            ("Anzahl", Value::U32(anzahl)),
        ],
    )
    .await;
}

/// Nach `AlsGelesen`/`Entfernen`/`AlleEntfernen` über D-Bus: Signale senden,
/// Popups der entfernten Einträge schließen, die Zentrale auffrischen.
async fn nach_aenderung(conn: &Connection, zustand: &Mutex<Zustand>, ids: Vec<u32>, weg: Vec<u32>) {
    melde_geaendert(conn, ids).await;
    melde_zaehler(conn, zustand).await;
    glib::idle_add_once(move || {
        popups::mit_dienst(|d| {
            for id in weg {
                d.schliesse(id, grund::GESCHLOSSEN);
            }
            d.zentrale_auffrischen();
        });
    });
}

/// Setzt „Nicht stören“ und meldet `PropertiesChanged` (außer beim
/// Eigenschafts-Setter, für den zbus das selbst tut). Im nächsten Leerlauf
/// des GTK-Threads folgen `ZustandGeaendert` und die Zentrale (Umschalter);
/// deshalb von beiden Seiten aufrufbar.
pub async fn nicht_stoeren_setzen(
    zustand: &Mutex<Zustand>,
    an: bool,
    conn: Option<&Connection>,
) -> bool {
    let geaendert = sperre(zustand).setze_nicht_stoeren(an);
    if !geaendert {
        return false;
    }
    log(&format!("Nicht stören {}", if an { "an" } else { "aus" }));
    if let Some(conn) = conn {
        eigenschaften_geaendert(conn, &[("NichtStoeren", Value::Bool(an))]).await;
    }
    glib::idle_add_once(move || {
        popups::mit_dienst(|d| {
            d.zaehler_melden();
            d.zentrale_auffrischen();
        });
    });
    true
}

/// `ZentraleOffen` hat sich geändert.
pub async fn melde_zentrale_offen(conn: &Connection, offen: bool) {
    eigenschaften_geaendert(conn, &[("ZentraleOffen", Value::Bool(offen))]).await;
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
            Ok(RequestNameReply::PrimaryOwner) => {}
            Ok(_) | Err(zbus::Error::NameTaken) => return Err(name_belegt(&conn).await),
            Err(e) => return Err(format!("Name {NAME} nicht belegbar: {e}")),
        }
        // Zweiter Name für Panel-Applet und `--zentrale` (ab 0.2.0); ohne ihn
        // läuft der Dienst trotzdem.
        match conn
            .request_name_with_flags(QUERENCIA, RequestNameFlags::DoNotQueue.into())
            .await
        {
            Ok(RequestNameReply::PrimaryOwner) => {}
            Ok(_) | Err(zbus::Error::NameTaken) => {
                log(&format!(
                    "Name {QUERENCIA} ist schon belegt; nur {NAME} gehört diesem Dienst"
                ));
            }
            Err(e) => log(&format!("Name {QUERENCIA} nicht belegbar: {e}")),
        }
        Ok(conn)
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

/// Ergebnis von `--zentrale`.
pub enum ZentraleAufruf {
    /// Der laufende Dienst zeigt das Fenster.
    Gezeigt,
    /// Kein Benachrichtigungsdienst: dieser Prozess wird der Dienst.
    KeinDienst,
    /// Ein anderer Dienst (oder eine ältere Fassung) hält
    /// org.freedesktop.Notifications: (Name, Version) aus GetServerInformation.
    Fremd(String, String),
}

/// `--zentrale`: ZentraleZeigen am Namen org.querencia.Benachrichtigungen.
/// `token` ist das xdg-activation-Token aus `XDG_ACTIVATION_TOKEN`.
pub fn client_zentrale(token: &str) -> Result<ZentraleAufruf, String> {
    zbus::block_on(async {
        let conn = Connection::session()
            .await
            .map_err(|e| format!("kein Sitzungsbus: {e}"))?;
        match conn
            .call_method(
                Some(QUERENCIA),
                PFAD,
                Some(QUERENCIA),
                "ZentraleZeigen",
                &(token,),
            )
            .await
        {
            Ok(_) => return Ok(ZentraleAufruf::Gezeigt),
            Err(zbus::Error::MethodError(name, _, _))
                if name.as_str().ends_with("ServiceUnknown")
                    || name.as_str().ends_with("NameHasNoOwner") => {}
            Err(e) => return Err(format!("Aufruf ZentraleZeigen fehlgeschlagen: {e}")),
        }
        // Erst nach dem Besitzer fragen: ein Aufruf würde sonst einen fremden
        // Dienst per D-Bus-Aktivierung starten (z. B. notification-daemon).
        let proxy = zbus::fdo::DBusProxy::new(&conn)
            .await
            .map_err(|e| e.to_string())?;
        let belegt = proxy
            .name_has_owner(zbus::names::BusName::try_from(NAME).expect("gültiger Name"))
            .await
            .map_err(|e| e.to_string())?;
        if !belegt {
            return Ok(ZentraleAufruf::KeinDienst);
        }
        let m = conn
            .call_method(Some(NAME), PFAD, Some(NAME), "GetServerInformation", &())
            .await
            .map_err(|e| format!("GetServerInformation fehlgeschlagen: {e}"))?;
        let (name, _, version, _): (String, String, String, String) =
            m.body().deserialize().map_err(|e| e.to_string())?;
        Ok(ZentraleAufruf::Fremd(name, version))
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
