use crate::browser::{web_url, Link, Page};
use ego_tree::iter::Edge;
use scraper::{ElementRef, Html, Selector};
use url::Url;

const LINK_LIMIT: usize = 256;
const LINE_LIMIT: usize = 512;

pub fn clean(s: &str) -> String {
    s.chars()
        .filter(|&c| {
            c.is_whitespace()
                || (!c.is_control()
                    && !matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn short(s: &str, limit: usize) -> (String, bool) {
    let mut chars = s.chars();
    let text = chars.by_ref().take(limit).collect();
    (text, chars.next().is_some())
}

pub fn append_line(lines: &mut Vec<String>, raw: &str, clipped: &mut bool) {
    let value = clean(raw);
    if value.is_empty() {
        return;
    }
    if lines.len() == LINE_LIMIT {
        *clipped = true;
        return;
    }
    let (line, cut) = short(&value, 2048);
    *clipped |= cut;
    lines.push(line);
}

fn boundary(name: &str) -> bool {
    matches!(
        name,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "br"
            | "dd"
            | "div"
            | "dl"
            | "dt"
            | "footer"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hr"
            | "li"
            | "main"
            | "nav"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "td"
            | "th"
            | "tr"
            | "ul"
    )
}

pub fn parse(source: &str, url: &Url, status: u16) -> Page {
    let html = Html::parse_document(source);
    let select = |s| Selector::parse(s).unwrap();
    let title = html
        .select(&select("title"))
        .next()
        .map(|e| clean(&e.text().collect::<String>()))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| url.host_str().unwrap_or("Document").into());
    let base = html
        .select(&select("base[href]"))
        .next()
        .and_then(|e| url.join(e.attr("href")?).ok())
        .and_then(|u| web_url(u.as_str()).ok())
        .unwrap_or_else(|| url.clone());
    let body = html
        .select(&select("body"))
        .next()
        .unwrap_or_else(|| html.root_element());
    let (title, mut truncated) = short(&title, 256);
    let mut lines = Vec::new();
    let mut links = Vec::new();
    let mut line = String::new();
    let mut hidden_root = None;
    let mut anchor: Option<(String, String, String)> = None;

    // Walk each edge once, including in deeply nested markup. Text and link labels
    // use the same visibility and whitespace rules.
    for edge in body.traverse() {
        let (node, entering) = match edge {
            Edge::Open(n) => (n, true),
            Edge::Close(n) => (n, false),
        };
        if let Some(id) = hidden_root {
            if !entering && id == node.id() {
                hidden_root = None;
            }
            continue;
        }
        if let Some(el) = ElementRef::wrap(node) {
            let name = el.value().name();
            if entering
                && (matches!(
                    name,
                    "head" | "script" | "style" | "template" | "noscript" | "svg" | "canvas"
                ) || el.attr("hidden").is_some()
                    || el
                        .attr("aria-hidden")
                        .is_some_and(|v| v.eq_ignore_ascii_case("true")))
            {
                hidden_root = Some(node.id());
                continue;
            }
            if boundary(name) {
                append_line(&mut lines, &line, &mut truncated);
                line.clear();
                if let Some((_, _, label)) = &mut anchor {
                    label.push(' ');
                }
            }
            if name == "a" {
                if entering {
                    anchor = el
                        .attr("href")
                        .and_then(|h| base.join(h).ok())
                        .and_then(|u| web_url(u.as_str()).ok())
                        .map(|u| {
                            (
                                u.to_string(),
                                clean(
                                    el.attr("aria-label")
                                        .or_else(|| el.attr("title"))
                                        .unwrap_or(""),
                                ),
                                String::new(),
                            )
                        });
                } else if let Some((url, fallback, label)) = anchor.take() {
                    if links.len() == LINK_LIMIT {
                        truncated = true;
                        continue;
                    }
                    let label = clean(&label);
                    let label = if label.is_empty() {
                        if fallback.is_empty() {
                            &url
                        } else {
                            &fallback
                        }
                    } else {
                        &label
                    };
                    let (label, cut) = short(label, 160);
                    truncated |= cut;
                    links.push(Link { label, url });
                }
            }
            if entering && name == "img" {
                if let Some(alt) = el.attr("alt") {
                    line.push_str(alt);
                    if let Some((_, _, label)) = &mut anchor {
                        label.push_str(alt);
                    }
                }
            }
        } else if entering {
            if let Some(t) = node.value().as_text() {
                line.push_str(t);
                if let Some((_, _, label)) = &mut anchor {
                    label.push_str(t);
                }
            }
        }
    }
    append_line(&mut lines, &line, &mut truncated);
    Page {
        url: url.to_string(),
        title,
        lines,
        links,
        truncated,
        status,
    }
}
