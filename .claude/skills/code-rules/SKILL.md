---
name: code-rules
description: The rules for changing Dailix's code, and the checks before a commit. Use before writing or changing any Rust in this repo, when reviewing it, and when the user asks for a refactor or a cleanup.
---

# Changing Dailix's code

`docs/CONTEXT.md` holds the why ("Published some day", "Don't inherit,
improve"); `docs/SHEET-CORE.md` the sheet core's design. This page is what
to hold in mind while writing, learned from the audit of 2026-11-05 (its
list: in Omnix's git history; what was kept: its `docs/omnix/ARCHITECTURE.md`): the cost was the same
fact written in several places, and rules living in UI handlers untested.

## Before writing

- **Logic in the core, the window only calls it.** Anything about the
  marking, the footage, a project's folder or `project.json` belongs in
  `crates/sheet` (`dailix_sheet`), with tests; the app (`crates/dailix`)
  carries out what the core decides (see `marking::key` and the pane). A
  command (`dailix …`) for what a test or an AI needs to drive.
- **Look for the place it already lives**, here and in Omnix
  (`../omnix-360-studio`): one clock (`dailix_sheet::clock`), one role
  table (`Sheet::all_roles`), `is_day`, `Layout` for every folder name,
  Omnix's `viewer::jpeg_frame` for frames, `rosmo360::app_dir` for folders.
  Omnix links `dailix_sheet`: grep both repos before calling a `pub` item
  unused.
- **The sheet file is kept as read**, byte for byte (key order, number
  form, unknown fields): changes only through ops on `Doc`, written by
  `Store` (fresh read, check, atomic write, `.bak`). `project.json` only
  through `dailix_sheet::project::update`, which keeps fields it doesn't
  know.

## While writing

- No personal data or machine paths in code, tests or comments; no film
  names: made-up samples in temp folders.
- Portable: `cfg` fallbacks; binaries with `EXE_SUFFIX`; ffmpeg through
  Omnix's library, never a bare `"ffmpeg"`.
- No help text in the window: hover hints (`tip`); the F1 keys list is the
  exception, kept in step with the keys (`help.rs`).
- Originals are never written: the sources place is read-only.
- Doc comments say why, on the item they describe; no dead code, no
  `allow(dead_code)`.
- Licence MIT OR Apache-2.0; no vendor SDKs.

## Before each commit

```sh
export PATH=$HOME/.cargo/bin:$PATH PKG_CONFIG_PATH=../omnix-360-studio/.localdeps/root/usr/lib/x86_64-linux-gnu/pkgconfig
cargo build -q && cargo test -q
(cd ../omnix-360-studio && cargo build -q && cargo test -q --lib)   # it links dailix_sheet
```

- One local commit per subtask, saying why; never push.
- A change the user will see: Omnix's roadmap card (its `roadmap` skill).
- `docs/CONTEXT.md` «Where it stands» rewritten, not appended.
