# Surfbird

<img align="right" src="surfbird.svg" alt="Silver parrot riding a cyan surfboard" width="120">

Surfbird is a hosted Rust reader browser for people and machines. Open a page in the terminal and an attached program can read it, follow its links or take you back. Both controls use one page, link list and history in memory.

This repository is the canonical source for the hosted reader. Submit hosted browser changes here. PCP-OS maintains the bare-metal guest, kernel networking, and QEMU replay, and carries a pinned hosted reader alongside them for integration checks. [Native PCP integration](https://github.com/KirtusJ/pcp-os/tree/master/apps/surfbird/guest)

This preview reads HTTP(S) HTML and plain text through the host operating system. It extracts text and links from HTML5 markup. CSS layout, JavaScript, forms, graphical rendering, extensions and a native PCP kernel build are outside this version. Pages that depend on styling or scripting can show different or incomplete content. Fragment URLs are retained in history. The reader currently opens them at the top of the document.

## Run

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
