//! Body-Markup nach der Freedesktop-Benachrichtigungsspezifikation sicher in
//! Pango-Markup umsetzen: `<b>`, `<i>`, `<u>` und `<a href="…">` bleiben,
//! `<img …>` wird entfernt, alles andere wird so escaped, dass es als Text
//! sichtbar ist und Pango den String immer parsen kann (Tags werden
//! geschlossen, falsch verschachtelte Tags als Text ausgegeben).

/// Wandelt den Body einer Meldung in gültiges Pango-Markup.
pub fn bereinige(body: &str) -> String {
    let mut aus = String::with_capacity(body.len() + 16);
    let mut offen: Vec<&'static str> = Vec::new();
    let mut rest = body;
    while let Some(pos) = rest.find('<') {
        text(&rest[..pos], &mut aus);
        rest = &rest[pos..];
        let Some(ende) = rest.find('>') else {
            // Kein schließendes '>' mehr: alles als Text.
            text(rest, &mut aus);
            rest = "";
            break;
        };
        let roh = &rest[1..ende];
        rest = &rest[ende + 1..];
        match tag(roh) {
            Tag::Oeffnen(name, attribute) => {
                aus.push('<');
                aus.push_str(name);
                aus.push_str(&attribute);
                aus.push('>');
                offen.push(name);
            }
            Tag::Schliessen(name) => {
                if let Some(index) = offen.iter().rposition(|o| *o == name) {
                    // Alles darüber zuerst schließen, dann dieses Tag.
                    while offen.len() > index {
                        let o = offen.pop().unwrap_or(name);
                        aus.push_str("</");
                        aus.push_str(o);
                        aus.push('>');
                    }
                } else {
                    text(&format!("<{roh}>"), &mut aus);
                }
            }
            Tag::Bild => {}
            Tag::Fremd => text(&format!("<{roh}>"), &mut aus),
        }
    }
    text(rest, &mut aus);
    while let Some(o) = offen.pop() {
        aus.push_str("</");
        aus.push_str(o);
        aus.push('>');
    }
    aus
}

/// Pango selbst kennt `<a>` nicht; GtkLabel übersetzt es vor dem Parsen.
/// Für eine Prüfung mit `pango::parse_markup` werden Links zu `<span>`.
pub fn pango_pruefbar(markup: &str) -> String {
    let mut aus = String::with_capacity(markup.len());
    let mut rest = markup;
    while let Some(p) = rest.find("<a href=\"") {
        aus.push_str(&rest[..p]);
        let nach = &rest[p..];
        match nach.find('>') {
            Some(e) => {
                aus.push_str("<span>");
                rest = &nach[e + 1..];
            }
            None => {
                aus.push_str(nach);
                rest = "";
            }
        }
    }
    aus.push_str(rest);
    aus.replace("</a>", "</span>")
}

enum Tag {
    Oeffnen(&'static str, String),
    Schliessen(&'static str),
    Bild,
    Fremd,
}

fn tag(roh: &str) -> Tag {
    let roh = roh.trim();
    if let Some(name) = roh.strip_prefix('/') {
        return match name.trim().to_ascii_lowercase().as_str() {
            "b" => Tag::Schliessen("b"),
            "i" => Tag::Schliessen("i"),
            "u" => Tag::Schliessen("u"),
            "a" => Tag::Schliessen("a"),
            _ => Tag::Fremd,
        };
    }
    let (name, attribute) = match roh.find(char::is_whitespace) {
        Some(p) => (&roh[..p], roh[p..].trim()),
        None => (roh, ""),
    };
    let name = name.to_ascii_lowercase();
    match name.as_str() {
        "b" | "i" | "u" if attribute.is_empty() => Tag::Oeffnen(
            match name.as_str() {
                "b" => "b",
                "i" => "i",
                _ => "u",
            },
            String::new(),
        ),
        "a" => match href(attribute) {
            Some(ziel) => Tag::Oeffnen("a", format!(" href=\"{}\"", escape_attribut(&ziel))),
            None => Tag::Fremd,
        },
        "img" => Tag::Bild,
        _ if name.ends_with('/') && matches!(name.trim_end_matches('/'), "img") => Tag::Bild,
        _ => Tag::Fremd,
    }
}

/// Liest `href="…"` oder `href='…'` aus den Attributen eines `<a>`-Tags.
/// Nur http, https, mailto und file werden als Ziel übernommen.
fn href(attribute: &str) -> Option<String> {
    let rest = attribute.trim_start();
    let rest = rest.strip_prefix("href")?.trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    let anfuehrung = rest.chars().next()?;
    if anfuehrung != '"' && anfuehrung != '\'' {
        return None;
    }
    let rest = &rest[1..];
    let ende = rest.find(anfuehrung)?;
    let ziel = rest[..ende].trim();
    let klein = ziel.to_ascii_lowercase();
    if ["http://", "https://", "mailto:", "file://"]
        .iter()
        .any(|p| klein.starts_with(p))
        && !ziel.chars().any(|c| c.is_control())
    {
        Some(ziel.to_string())
    } else {
        None
    }
}

fn escape_attribut(s: &str) -> String {
    let mut aus = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => aus.push_str("&amp;"),
            '<' => aus.push_str("&lt;"),
            '>' => aus.push_str("&gt;"),
            '"' => aus.push_str("&quot;"),
            '\'' => aus.push_str("&apos;"),
            c => aus.push(c),
        }
    }
    aus
}

/// Escaped Text; die fünf XML-Entitäten sowie numerische Zeichenreferenzen
/// bleiben erhalten, jedes andere `&` wird zu `&amp;`.
fn text(s: &str, aus: &mut String) {
    let mut rest = s;
    while let Some(pos) = rest.find(['&', '<', '>']) {
        aus.push_str(&rest[..pos]);
        let c = rest.as_bytes()[pos];
        rest = &rest[pos + 1..];
        match c {
            b'<' => aus.push_str("&lt;"),
            b'>' => aus.push_str("&gt;"),
            _ => {
                if let Some(len) = entitaet(rest) {
                    aus.push('&');
                    aus.push_str(&rest[..len]);
                    rest = &rest[len..];
                } else {
                    aus.push_str("&amp;");
                }
            }
        }
    }
    aus.push_str(rest);
}

/// Länge einer gültigen Entität (ohne führendes `&`, mit `;`), falls `rest`
/// mit einer beginnt.
fn entitaet(rest: &str) -> Option<usize> {
    let ende = rest.find(';')?;
    let name = &rest[..ende];
    let ok = matches!(name, "amp" | "lt" | "gt" | "quot" | "apos")
        || name.strip_prefix("#x").is_some_and(|h| {
            !h.is_empty() && h.len() <= 6 && h.chars().all(|c| c.is_ascii_hexdigit())
        })
        || name.strip_prefix('#').is_some_and(|d| {
            !d.is_empty() && d.len() <= 7 && d.chars().all(|c| c.is_ascii_digit())
        });
    ok.then_some(ende + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pango_ok(markup: &str) -> bool {
        gtk4::pango::parse_markup(&pango_pruefbar(markup), '\0').is_ok()
    }

    #[test]
    fn pruefbar_ersetzt_links() {
        assert_eq!(
            pango_pruefbar(r#"x <a href="https://a.de">l</a> y"#),
            "x <span>l</span> y"
        );
        assert_eq!(pango_pruefbar("<b>kein Link</b>"), "<b>kein Link</b>");
    }

    #[test]
    fn erlaubte_tags_bleiben() {
        assert_eq!(
            bereinige("<b>fett</b> <i>kursiv</i> <u>unter</u>"),
            "<b>fett</b> <i>kursiv</i> <u>unter</u>"
        );
        assert!(pango_ok(&bereinige(
            "<b>fett</b> <i>kursiv</i> <u>unter</u>"
        )));
    }

    #[test]
    fn grossschreibung_wird_normalisiert() {
        assert_eq!(bereinige("<B>x</B>"), "<b>x</b>");
    }

    #[test]
    fn links_mit_sicherem_ziel() {
        let aus = bereinige(r#"<a href="https://example.org/?a=1&b=2">Link</a>"#);
        assert_eq!(
            aus,
            r#"<a href="https://example.org/?a=1&amp;b=2">Link</a>"#
        );
        assert!(pango_ok(&aus));
    }

    #[test]
    fn links_mit_fremdem_schema_werden_text() {
        let aus = bereinige(r#"<a href="javascript:alert(1)">x</a>"#);
        assert_eq!(aus, r#"&lt;a href="javascript:alert(1)"&gt;x&lt;/a&gt;"#);
        assert!(pango_ok(&aus));
    }

    #[test]
    fn fremde_tags_werden_escaped() {
        let aus = bereinige("<script>böse</script> <span color=\"red\">rot</span>");
        assert_eq!(
            aus,
            "&lt;script&gt;böse&lt;/script&gt; &lt;span color=\"red\"&gt;rot&lt;/span&gt;"
        );
        assert!(pango_ok(&aus));
    }

    #[test]
    fn bild_tag_wird_entfernt() {
        assert_eq!(
            bereinige(r#"vor <img src="x.png" alt="x"/> nach"#),
            "vor  nach"
        );
        assert_eq!(bereinige(r#"<img src="x.png">"#), "");
    }

    #[test]
    fn kleiner_als_in_text() {
        let aus = bereinige("3 < 5 und 7 > 2");
        assert_eq!(aus, "3 &lt; 5 und 7 &gt; 2");
        assert!(pango_ok(&aus));
    }

    #[test]
    fn unvollstaendiges_tag_am_ende() {
        let aus = bereinige("Text <b");
        assert_eq!(aus, "Text &lt;b");
        assert!(pango_ok(&aus));
    }

    #[test]
    fn offene_tags_werden_geschlossen() {
        let aus = bereinige("<b>fett <i>beides");
        assert_eq!(aus, "<b>fett <i>beides</i></b>");
        assert!(pango_ok(&aus));
    }

    #[test]
    fn falsch_verschachtelt() {
        let aus = bereinige("<b>a <i>b</b> c</i>");
        assert_eq!(aus, "<b>a <i>b</i></b> c&lt;/i&gt;");
        assert!(pango_ok(&aus));
    }

    #[test]
    fn schliessen_ohne_oeffnen() {
        let aus = bereinige("x</b>y");
        assert_eq!(aus, "x&lt;/b&gt;y");
        assert!(pango_ok(&aus));
    }

    #[test]
    fn entitaeten_bleiben_rest_wird_escaped() {
        let aus = bereinige("Tom &amp; Jerry &lt;3 &#8364; &#x20AC; Kaffee & Kuchen &bogus; &");
        assert_eq!(
            aus,
            "Tom &amp; Jerry &lt;3 &#8364; &#x20AC; Kaffee &amp; Kuchen &amp;bogus; &amp;"
        );
        assert!(pango_ok(&aus));
    }

    #[test]
    fn tags_mit_attributen_an_b_sind_fremd() {
        let aus = bereinige("<b onclick=\"x\">a</b>");
        assert_eq!(aus, "&lt;b onclick=\"x\"&gt;a&lt;/b&gt;");
        assert!(pango_ok(&aus));
    }

    #[test]
    fn zeilenumbrueche_und_umlaute_bleiben() {
        let aus = bereinige("Grüße\naus Köln – 10 €");
        assert_eq!(aus, "Grüße\naus Köln – 10 €");
    }

    #[test]
    fn leerer_body() {
        assert_eq!(bereinige(""), "");
    }

    #[test]
    fn alles_parst_in_pango() {
        for s in [
            "<b><b>doppelt</b></b>",
            "<a href='http://x.de'>a</a>",
            "<a href=\"\">leer</a>",
            "<<<>>>",
            "<b>&</b>",
            "<u><a href=\"mailto:a@b.de\">m</a></u>",
            "<i/>",
            "< b>abstand</b>",
        ] {
            let aus = bereinige(s);
            assert!(pango_ok(&aus), "{s:?} -> {aus:?}");
        }
    }
}
