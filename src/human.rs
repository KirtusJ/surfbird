use crate::{
    browser::Command,
    session::{dispatch, Bus},
};
use serde_json::Value;
use std::io::{self, BufRead, Write};
const PER_PAGE: usize = 20;
fn show(out: &mut impl Write, state: &Value, offset: usize, ansi: bool) -> io::Result<()> {
    let page = &state["page"];
    let lines = page["lines"].as_array().unwrap();
    let links = page["links"].as_array().unwrap();
    let (cyan, silver, reset) = if ansi {
        ("\x1b[96m", "\x1b[37m", "\x1b[0m")
    } else {
        ("", "", "")
    };
    let title = page["title"].as_str().unwrap();
    let url = page["url"].as_str().unwrap();
    let revision = &state["revision"];
    writeln!(
        out,
        "\n{cyan}  __,>   SURFBIRD{reset}  {silver}{title}  ·  revision {revision}{reset}"
    )?;
    writeln!(out, "{cyan} ~====~{reset}  {url}")?;
    for l in lines.iter().skip(offset).take(PER_PAGE) {
        writeln!(out, "  {}", l.as_str().unwrap())?;
    }
    if lines.len() > offset + PER_PAGE {
        writeln!(out, "  …  more")?;
    }
    if page["truncated"] == true {
        writeln!(out, "  Document reached the reader display limits.")?;
    }
    if !links.is_empty() {
        writeln!(out, "{cyan}  Links{reset}")?;
        for (i, l) in links.iter().take(10).enumerate() {
            writeln!(out, "  {:>2}  {}", i + 1, l["label"].as_str().unwrap())?;
        }
        if links.len() > 10 {
            writeln!(out, "  links  lists all {} destinations", links.len())?;
        }
    }
    writeln!(
        out,
        "  open URL  ·  NUMBER follows a link  ·  back / forward  ·  read  ·  quit"
    )
}
pub fn run(mut input: impl BufRead, mut out: impl Write, bus: &Bus, ansi: bool) -> io::Result<()> {
    let mut state = dispatch(bus, Command::Read).map_err(io::Error::other)?;
    let mut offset = 0;
    show(&mut out, &state, offset, ansi)?;
    loop {
        write!(out, "surf> ")?;
        out.flush()?;
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            return Ok(());
        }
        let line = line.trim();
        let cmd = match line {
            "quit" | "q" => return Ok(()),
            "read" | "" => Command::Read,
            "back" => Command::Back,
            "forward" => Command::Forward,
            "reload" => Command::Reload,
            "links" => {
                state = dispatch(bus, Command::Read).map_err(io::Error::other)?;
                for (i, l) in state["page"]["links"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                {
                    writeln!(
                        out,
                        "  {}  {}  {}",
                        i + 1,
                        l["label"].as_str().unwrap(),
                        l["url"].as_str().unwrap()
                    )?;
                }
                continue;
            }
            "more" => {
                let fresh = dispatch(bus, Command::Read).map_err(io::Error::other)?;
                if fresh["revision"] != state["revision"] {
                    offset = 0;
                } else if offset + PER_PAGE < fresh["page"]["lines"].as_array().unwrap().len() {
                    offset += PER_PAGE;
                }
                state = fresh;
                show(&mut out, &state, offset, ansi)?;
                continue;
            }
            _ if line.starts_with("open ") => Command::Open {
                url: line[5..].trim().into(),
            },
            _ if line.starts_with("https://") || line.starts_with("http://") => {
                Command::Open { url: line.into() }
            }
            _ => match line.parse::<usize>() {
                Ok(link) => Command::Follow {
                    link,
                    revision: state["revision"].as_u64(),
                },
                Err(_) => {
                    writeln!(out,"Use open URL, a link number, back, forward, reload, read, more, links or quit.")?;
                    continue;
                }
            },
        };
        match dispatch(bus, cmd) {
            Ok(v) => {
                state = v;
                offset = 0;
                show(&mut out, &state, offset, ansi)?;
            }
            Err(e) => writeln!(out, "Navigation: {}", crate::document::clean(&e))?,
        }
    }
}
