use crate::document::{append_line, parse};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use url::Url;

const DOCUMENT_LIMIT: u64 = 2 * 1024 * 1024;
const HISTORY_LIMIT: usize = 24;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Link {
    pub label: String,
    pub url: String,
}
#[derive(Debug, Serialize, PartialEq)]
pub struct Page {
    pub url: String,
    pub title: String,
    pub lines: Vec<String>,
    pub links: Vec<Link>,
    pub truncated: bool,
    pub status: u16,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Open {
        url: String,
    },
    Follow {
        link: usize,
        #[serde(default)]
        revision: Option<u64>,
    },
    Back,
    Forward,
    Reload,
    Read,
}
pub struct Browser {
    agent: ureq::Agent,
    history: Vec<Page>,
    at: usize,
    pub revision: u64,
}
impl Browser {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(12)))
            .max_redirects(0)
            .http_status_as_error(false)
            .user_agent("Surfbird/0.1 (Rust reader)")
            .build()
            .into();
        let start = Page {
            url: "surfbird:home".into(),
            title: "Surfbird".into(),
            status: 0,
            truncated: false,
            lines: vec!["Open an address or follow a link.".into()],
            links: vec![
                Link {
                    label: "Example Domain".into(),
                    url: "https://example.com/".into(),
                },
                Link {
                    label: "The Rust Programming Language".into(),
                    url: "https://www.rust-lang.org/".into(),
                },
            ],
        };
        Self {
            agent,
            history: vec![start],
            at: 0,
            revision: 0,
        }
    }
    pub fn page(&self) -> &Page {
        &self.history[self.at]
    }
    pub fn navigation(&self) -> (bool, bool) {
        (self.at > 0, self.at + 1 < self.history.len())
    }
    pub fn act(&mut self, command: Command) -> Result<(), String> {
        match command {
            Command::Read => return Ok(()),
            Command::Back => {
                if self.at == 0 {
                    return Err("Already at the beginning of history".into());
                }
                self.at -= 1;
            }
            Command::Forward => {
                if self.at + 1 == self.history.len() {
                    return Err("Already at the end of history".into());
                }
                self.at += 1;
            }
            Command::Reload => {
                if self.page().url == "surfbird:home" {
                    return Ok(());
                }
                let page = fetch(&self.agent, web_url(&self.page().url)?)?;
                self.history[self.at] = page;
            }
            navigation => {
                let address = match navigation {
                    Command::Open { url } => url,
                    Command::Follow { link, revision } => {
                        if revision.is_some_and(|r| r != self.revision) {
                            return Err(
                                "Page changed. Read the current page before selecting its link."
                                    .into(),
                            );
                        }
                        link.checked_sub(1)
                            .and_then(|n| self.page().links.get(n))
                            .ok_or("Link number is outside this page")?
                            .url
                            .clone()
                    }
                    _ => unreachable!(),
                };
                let address = web_url(&address)?;
                let mut here = Url::parse(&self.page().url).unwrap();
                here.set_fragment(None);
                let mut dest = address.clone();
                dest.set_fragment(None);
                let page = if address.fragment().is_some() && here == dest {
                    let prev = self.page();
                    Page {
                        url: address.to_string(),
                        title: prev.title.clone(),
                        lines: prev.lines.clone(),
                        links: prev.links.clone(),
                        status: prev.status,
                        truncated: prev.truncated,
                    }
                } else {
                    fetch(&self.agent, address)?
                };
                self.history.truncate(self.at + 1);
                self.history.push(page);
                if self.history.len() > HISTORY_LIMIT {
                    self.history.remove(0);
                }
                self.at = self.history.len() - 1;
            }
        }
        self.revision += 1;
        Ok(())
    }
}
pub(crate) fn web_url(raw: &str) -> Result<Url, String> {
    if raw.len() > 8192 {
        return Err("Address exceeds 8192 bytes".into());
    }
    let u = Url::parse(raw).map_err(|_| "Enter a full HTTP or HTTPS URL")?;
    // Percent encoding can expand even an input that passed the raw limit.
    if u.as_str().len() > 8192 {
        return Err("Canonical address exceeds 8192 bytes".into());
    }
    if !matches!(u.scheme(), "http" | "https")
        || !u.has_host()
        || !u.username().is_empty()
        || u.password().is_some()
    {
        return Err("Only HTTP(S) addresses without embedded credentials can be opened".into());
    }
    Ok(u)
}
fn fetch(agent: &ureq::Agent, mut url: Url) -> Result<Page, String> {
    for _ in 0..=6 {
        let mut res = agent
            .get(url.as_str())
            .header("Accept", "text/html, text/plain;q=0.8")
            .call()
            .map_err(|e| format!("Fetch failed: {e}"))?;
        let status = res.status().as_u16();
        if [301, 302, 303, 307, 308].contains(&status) {
            let loc = res
                .headers()
                .get("location")
                .and_then(|h| h.to_str().ok())
                .ok_or("Redirect has no usable destination")?;
            url = web_url(
                url.join(loc)
                    .map_err(|_| "Invalid redirect destination")?
                    .as_str(),
            )?;
            continue;
        }
        let mime = res
            .body()
            .mime_type()
            .unwrap_or("text/html")
            .to_ascii_lowercase();
        if mime != "text/html" && mime != "text/plain" {
            return Err(format!("Unsupported content type: {mime}"));
        }
        let text = res
            .body_mut()
            .with_config()
            .limit(DOCUMENT_LIMIT)
            .read_to_string()
            .map_err(|e| format!("Read failed: {e}"))?;
        if mime == "text/plain" {
            let mut clipped = false;
            let mut lines = Vec::new();
            for l in text.lines() {
                append_line(&mut lines, l, &mut clipped);
            }
            return Ok(Page {
                url: url.to_string(),
                title: url.host_str().unwrap_or("Document").into(),
                lines,
                links: vec![],
                status,
                truncated: clipped,
            });
        }
        return Ok(parse(&text, &url, status));
    }
    Err("Too many redirects".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn html_reader() {
        let u = web_url("https://site.test/a/start").unwrap();
        let p=parse("<title>A &amp; B</title><base href='../'> <p>Hello <em>reader</em>!</p><script>skip()</script><p hidden>secret</p><a href='next'>Go <b>there</b></a><a href='javascript:noop()'>invalid</a>",&u,200);
        assert_eq!(p.title, "A & B");
        assert_eq!(p.lines[0], "Hello reader!");
        assert!(!p.lines.join(" ").contains("secret"));
        assert_eq!(p.links[0].url, "https://site.test/next");
        assert_eq!(p.links[0].label, "Go there");
        assert_eq!(p.links.len(), 1);
    }
    #[test]
    fn controls() {
        assert_eq!(crate::document::clean("x\x1b[31m\u{202e}y"), "x[31my");
        assert!(web_url("file:///a").is_err());
        assert!(web_url("https://user:pw@site.test/").is_err());
        assert!(web_url(&format!("https://example.org/{}", "é".repeat(1600))).is_err());
    }
}
