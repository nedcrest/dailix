---
name: code-rules
description: The rules for changing Dailix's code, and the checks before a commit. Use before writing or changing any Rust in this repo, when reviewing it, and when the user asks for a refactor or a cleanup.
---

# Changing Dailix's code

`docs/CONTEXT.md` holds the why ("Published some day", "Don't inherit,
improve"); Omnix's `docs/SHEET-CORE.md` the sheet core's design. This page is what
to hold in mind while writing, learned from the audit of 2026-11-05 (its
list: in Omnix's git history; what was kept: its `docs/omnix/ARCHITECTURE.md`): the cost was the same
fact written in several places, and rules living in UI handlers untested.

## Before writing

- **Logic in the core, the window only calls it.** Anything about the
  marking, the footage, a project's folder or `project.json` belongs in
  the sheet core (`dailix_sheet`, `crates/sheet` in Omnix's repo), with
  tests; the app (`crates/dailix`)
  carries out what the core decides (see `marking::key` and the pane). A
  command (`dailix …`) for what a test or an AI needs to drive.
- **Look for the place it already lives**, here and in Omnix
  (`../omnix-360-studio`): one clock (`dailix_sheet::clock`), one role
  table (`Sheet::all_roles`), `is_day`, `Layout` for every folder name,
  Omnix's `omnix::viewer::jpeg_frame` for frames, `omnix::app_dir` for folders.
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

## Habits that grew into a refactor

Omnix's timeline took a two-day cleanup (2026-11-06/07) for five habits;
Dailix's window is young, so hold to their opposite from the start (the
long form, with examples: Omnix's `.claude/skills/code-rules`).

- **One question, one answer**: what's under the pointer, what a key
  does, which panel owns a place: one function, read by every caller.
- **The second copy is the moment to share**: a card, a tag, a strip
  drawn twice gets its helper in the same commit.
- **A view shows the model; it doesn't restate it**: it holds the
  sheet's or footage's item and only what the window loads (posters,
  frames), never a field copied under a new name.
- **A derived value is a method** (a segment's length, a day's clips
  rated): written once, in the core where it can be.
- **Don't store what follows from something else**: store what the user
  chose, derive the rest.
- **Functions stay short** (~80 lines, a match arm ~15): split before
  adding. **Clean as you go**: a broken rule in what you touch is fixed
  first, in its own commit.
- **Sizes relative to the box**, tried at the smallest and largest window
  and scale.

## Before each commit

```sh
# Where ALSA's headers aren't installed, PKG_CONFIG_PATH as Omnix's docs/omnix/LOCAL.md says.
cargo build -q && cargo test -q    # never commit what doesn't build
(cd ../omnix-360-studio && cargo build -q && cargo test -q --workspace)   # dailix_sheet lives there
```

- One local commit per subtask, saying why; never push.
- A change the user will see: Omnix's roadmap card (its `roadmap` skill).
- `docs/CONTEXT.md` «Where it stands» rewritten, not appended.
