# Surfbird

<img align="right" src="surfbird.svg" alt="Silver parrot riding a cyan surfboard" width="120">

Surfbird runs as a hosted terminal reader, an optional Windows browser window, and a separate graphical guest inside PCP-OS.

| What you want to run or change | Where it lives |
| --- | --- |
| **Hosted reader.** Open HTTP(S) pages in a terminal. A program can use the local JSON API to read the same page, follow links and share its history. | This repository owns the [Rust source](src/) and [tests](tests/). Submit hosted changes here. |
| **Windows browser window.** Navigate websites and play video with their own controls in a Surfbird window. | This repository owns the [WebView2 host](src/window.rs). See [Run the Windows window](#run-the-windows-window). |
| **Native graphical guest.** Read a fixture page on the PCP-OS display. Press `1` or `2` to follow a link and `B` to go back. | PCP-OS owns the [guest source, controls and replay instructions](https://github.com/KirtusJ/pcp-os/tree/master/apps/surfbird/guest). |
| **Native networking and integration.** Inspect guest DNS and TCP traffic, kernel device access or a QEMU run. | See the [DNS captures](https://github.com/KirtusJ/pcp-os/blob/master/apps/surfbird/guest/DNS.md), [TCP replay](https://github.com/KirtusJ/pcp-os/blob/master/apps/surfbird/guest/TCP.md) and [pinned hosted snapshot](https://github.com/KirtusJ/pcp-os/tree/master/apps/surfbird). |

## Run the Windows window

Build with Rust 1.91 or newer on Windows and the [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/). Open a YouTube watch page directly, then press Play in the page's player.

```sh
cargo run --locked --features window -- --window "https://www.youtube.com/watch?v=jNQXAC9IVRw"
```

Use the address bar, Back, Forward and Reload to navigate in the same window. WebView2 renders the page, runs its JavaScript and media, and supports keyboard zoom. The browser displays the committed address when a site changes location, including navigation within a single-page app. The engine stores its profile in `%LOCALAPPDATA%\Surfbird\WebView2`. This mode blocks popup windows and file downloads. The terminal's machine API controls the terminal session.

The standard Windows MSVC build statically links Microsoft's WebView2 loader. When distributing a Windows GNU build, place the matching architecture's `WebView2Loader.dll` next to the executable. The locked `webview2-com-sys` build supplies it under Cargo's `build/webview2-com-sys-*/out/<architecture>/` directory for that build profile. [Microsoft's distribution guide](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution)

## Run

The terminal reader extracts text and links from HTTP(S) HTML5 pages. It reads plain text as well. Pages that depend on layout or scripting can show incomplete content in this mode. Fragment URLs are retained in history and open at the top of the document.

From this directory with Rust 1.91 or newer, run:

```sh
cargo run --locked -- https://doc.rust-lang.org/book/
```

Enter a link number to follow it, `links` to see every numbered destination, `more` to page through the text, or `open URL` to visit another address. Use `back`, `forward`, `reload`, `read` and `quit` to control the session. Enter `read` after another client navigates. A number typed against an outdated page is rejected. `NO_COLOR` turns off terminal colors.

## Share the session

In the first terminal, start the browser with a loopback listener. Port zero asks the OS for a free port.

```sh
cargo run --locked -- --listen 127.0.0.1:0 https://doc.rust-lang.org/book/
```

In a second terminal, replace `PORT` with the number printed after `Machine session`.

```sh
cargo run --locked -- --machine 127.0.0.1:PORT
```

Type one JSON command per line. For example, read the current page:

```json
{"op":"read"}
```

A successful response returns `ok: true` and a `state` containing the current revision, back/forward availability and page. The page contains its final URL, HTTP status, title, text lines, links and a truncation flag. A failed navigation returns `ok: false` and an error, and preserves the current page. To follow the third link from revision 1, enter:

```json
{"op":"follow","link":3,"revision":1}
```

Use the revision from your own read. A client can omit the revision to act on whichever page is current. Other operations are `open` (with a `url`), `back`, `forward` and `reload`. Any program can speak newline-delimited UTF-8 JSON directly over the same socket. Ordinary startup leaves the listener off. When enabled, other processes able to reach that loopback port can read and control the session. The protocol has no authentication. Exit the terminal browser to end the shared session.

Surfbird keeps up to 24 history entries and follows up to six HTTP redirects per navigation. Its document reader accepts up to 2 MiB per response and reports clipping for display limits. Closing the browser discards session history.

The machine client allows up to 16 MiB per reply, including its final newline, and 120 seconds from sending the command to receiving that newline. It exits with an error if a reply is incomplete, too large or late. A timeout can occur while the browser is still processing a command. Reconnect and read the current state before retrying a navigation.

## Verify

```sh
cargo test --locked
cargo test --locked --test shared_session rust_book_over_both_interfaces -- --ignored --nocapture
```

The offline suite starts the executable, serves a redirect and two linked pages, then exercises terminal navigation and the socket together. It covers relative links under an HTML base, stale link numbers, failed navigation, back/forward/reload and rejection of an HTTP request at the machine port.

The opt-in HTTPS test opens the Rust Book and follows its Installation link, first through terminal input and then through a separate Surfbird machine-client process. On 14 November 2026 both visits returned the same destination, title, 57 text lines and 20 links. Back restored the book’s front page from each interface. The test compares the current pages and allows the upstream book to change. [Rust Book](https://doc.rust-lang.org/book/) · [Installation](https://doc.rust-lang.org/book/ch01-01-installation.html)

## Implementation

The Rust session owner serializes navigation and commits a fetched document only on success. The loopback service and terminal submit commands to that owner. HTML parsing uses `scraper` and `html5ever`; URL resolution uses the `url` crate. The reader processes visible HTML text in document order and does not evaluate scripts. HTTP and HTTPS transport uses `ureq` and its default rustls backend. [scraper](https://docs.rs/scraper/) · [URL Standard](https://url.spec.whatwg.org/) · [ureq](https://docs.rs/ureq/)

## Origin and rights

The two original hosted-reader commits were extracted from the PCP-OS history with their authors and messages preserved. The initial code, tests, Cargo manifest, lockfile and mascot match the hosted files at the verified PCP-OS #106 baseline. [Initial hosted work](https://github.com/KirtusJ/pcp-os/pull/105) · [Baseline commit](https://github.com/KirtusJ/pcp-os/commit/bc4969503b2e8ac407311c535754a1fb93ab071f)

Proprietary. All rights reserved. Copyright © 2026 Kirtus Justus. Dependency licenses are recorded by their respective packages. [Upstream rights notice](https://github.com/KirtusJ/pcp-os/blob/bc4969503b2e8ac407311c535754a1fb93ab071f/README.md#rights)
