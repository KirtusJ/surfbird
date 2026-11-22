# Catalogue of this repository

Every entry at the top of the repository, with one line on what it is. `tests/check_catalog.py` fails when a tracked top-level path is missing here or a path listed here no longer exists, so this page cannot fall behind the tree. The check is PCP OS's own, taken from `pcp-os` at a pinned commit.

"In the release" says whether the path's content is part of what a person boots or installs with PCP OS. "not determined" means nobody has yet established it; it is not a quiet yes.

| Path | What it is | In the release |
|---|---|---|
| `.github` | Continuous integration: the catalogue check. | no |
| `Cargo.lock` | The exact versions of what the program is built from. | no |
| `Cargo.toml` | The Rust package. | no |
| `CATALOG.md` | This page. | no |
| `README.md` | What Surfbird is, where each part lives and how to run it. | no |
| `src` | The hosted reader and the optional Windows browser window, in Rust. PCP OS pins a snapshot of the hosted reader; the native guest is in `pcp-os`. | not determined |
| `surfbird.svg` | The mark: a silver parrot riding a cyan surfboard. | no |
| `tests` | Tests of the hosted reader's replies and shared session, and this catalogue's check. | no |
