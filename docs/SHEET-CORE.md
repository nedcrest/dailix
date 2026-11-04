# The contact sheet's core: one way to read and change `notes.json`

Design for the user's approval before code (2026-11-04), as Omnix's
`OPS-API.md` was for projects. The user: start the move to Rust with the
contact sheet's core; "a core API for the contact sheet too, so changes
are easier in both places at once".

## Why a core, and why first

- **Three writers, no shared rules.** The web sheet saves the whole file
  from the browser's memory; Omnix's `notes.rs` patches it by hand
  (segments, roles, notes, delete); footage-agent's Python reads it in
  four places (`query.py` twice, `todo.py`, `export/omnix.py`). Each
  knows the format its own way. Two writers already needed a guard
  against stale saves.
- **No ids.** A mark is a list item. Omnix addresses one by its place
  and checks it's unchanged (`t`, `end`) before writing; the AI (the
  brainstorm in footage-agent's `CONTACT-SHEET-NEXT.md`, D3) and a cut's
  link back to its mark need a stable id.
- **What comes next stands on it:** the studio's own window (every edit
  through the core), Omnix (the same calls), the AI (operations, a
  schema, a text view), tests (operations replayed, the file checked).

## The format today (read from the code and the Kos file, 2026-11-04)

```text
{
  "days":  { "<YYYY-MM-DD>": { "note": "<the day's story>" } },
  "clips": { "<day>/<file>": {
      "rating": 2 | 1 | 0 | null,   // ✓ take, ? think, ✕ junk, unrated
      "note":   "<about the clip>",
      "audio":  "dnn6",             // the cleaned voice chosen (voice/<day>/<clip>/<name>.flac); absent: the clip's own
      "marks": [
        { "t": 21.8, "end": 25.0, "role": "broll", "note": "…" },   // a segment
        { "t": 7.0, "note": "…" },                                  // a point (first version's)
        { "t": 26, "kind": "flag", "role": "cut", "note": "…" }     // a flag: a note to self, never material
      ] } },
  "roles": [ { "key": "x_…", "title": "…", "en": "…", "hint": "…" } ],  // the user's own, beside the eight built in
  "ui":    { "shut": ["<day>", …] }   // the sheet's view state: days folded
}
```

Facts that shape the design:
- The sheet writes it with Python's `json.dumps(indent=2,
  ensure_ascii=False)`, no trailing newline; the Kos file is exactly that
  form. Times are rounded to a tenth in the browser, so 27 of them are
  integers (`26`, not `26.0`).
- A segment shorter than 0.4 s is saved as a point (the sheet's rule).
- A rated ✓ clip with no segments goes into a stringout whole; flags never
  count as marking (`query.py`).
- The voice clean-up state lives on disk (`voice/…/*.flac`), not here;
  only the choice (`audio`) does.
- Fields this design doesn't know must survive any write: the web sheet
  keeps adding them (`audio` came that way).

## The design

### 1. The model: typed, with the unknown kept

```rust
pub struct Sheet {
    pub days: IndexMap<String, Day>,          // in the order read
    pub clips: IndexMap<ClipKey, Clip>,
    pub roles: Vec<Role>,                     // skipped when empty
    pub ui: Value,                            // the sheet's view state, passed through
    #[serde(flatten)] pub rest: Map<String, Value>,
}
pub struct Clip { rating: Option<Rating>, note: String, audio: Option<String>,
                  marks: Vec<Mark>, #[serde(flatten)] rest: Map<String, Value> }
pub struct Mark { id: MarkId, t: Time, end: Option<Time>, kind: MarkKind,  // Segment | Point | Flag
                  role: Option<String>, note: String, #[serde(flatten)] rest: Map<String, Value> }
```

- `rest` on every level: a field the core doesn't know is read and
  written back untouched.
- `Time` writes an integral value as an integer (`26`, not `26.0`) and
  keeps the order of keys as read, so a file read and written unchanged
  is byte for byte the same (a test on a synthetic sheet; checked once
  by hand on a copy of Kos's).
- `MarkKind` is derived on read (`kind: "flag"`, else `end` present or
  not) and written back in the sheet's own shape, not as a new field.

### 2. Ids for marks

A new optional field `id` on each mark: short, unique in the file
(`m1`, `m2`…), given on load to marks without one and kept forever after.
The web sheet keeps fields it doesn't know on the objects it holds, so
ids survive its saves; a mark it creates comes without one and gets one
at the core's next load. Python readers use `.get`, so they don't mind.

### 3. Operations: a closed enum, as Omnix's

```rust
#[serde(tag = "area", rename_all = "snake_case", deny_unknown_fields)]
pub enum SheetOp { Mark(MarkOp), Clip(ClipOp), Day(DayOp), Role(RoleOp) }

pub enum MarkOp {
    Add    { clip: ClipKey, t: f64, end: Option<f64>, flag: bool, role: Option<String>, note: String },
    Move   { mark: MarkId, t: f64, end: Option<f64> },
    Role   { mark: MarkId, role: Option<String> },
    Note   { mark: MarkId, note: String },
    Remove { mark: MarkId },
}
pub enum ClipOp { Rate { clip: ClipKey, rating: Option<Rating> }, Note { clip: ClipKey, note: String },
                  Voice { clip: ClipKey, audio: Option<String> } }
pub enum DayOp  { Note { day: String, note: String } }
pub enum RoleOp { Add { title: String, en: String, hint: String } }   // key made as the sheet makes it
```

- Absolute values (a role, a note, a span), never "change by": an op
  replayed gives the same file (Omnix's «from the original» rule).
- Addressed by id and key, no hidden context (no "the picked mark").
- `apply` either does the whole op or nothing, and says what it made
  (`Outcome { made: Option<MarkId> }`) or why not (`OpError`: not found,
  invalid, and the check's message).
- The sheet's `ui` has no ops: it is the view's, not the marking.

### 4. `check`: what a sheet must be after every op

- every mark: `t ≥ 0`; a segment's `end > t + 0.4`; a flag has no end;
- ids unique; a role is one of the eight or the sheet's own;
- a rating is 0, 1, 2 or none; a day key is a date; a clip key is
  `<day>/<file>` with that day;
- (with a clip's length known, from the sidecar) marks inside the clip.

### 5. Undo and the file

- `Edit`: the sheet plus snapshot undo with `begin` / `commit`, as
  Omnix's (a drag on a strip is one step; nothing changed, no step).
- `Store`: the file on disk. Every write reads the file afresh, applies
  the ops given (id-addressed, so they land on whatever is there now),
  checks, writes atomically with the last version beside it (`.bak`).
  While the web sheet lives it can still save the whole file; its
  server refuses a stale save (footage-agent `3a67dba`).

### 6. CLI, schema, a text view

The studio's binary, CLI-only until it has a window:
- `rushix sheet apply <notes.json> <ops.json>`: the ops, checked,
  written; one line per outcome;
- `rushix sheet describe <notes.json> [--day D]`: a line per mark
  (day, clip, span, role, note) and per rated clip: what the AI reads in
  place of the JSON (brainstorm D12);
- `rushix sheet schema`: the JSON Schema of the file and of the
  ops (`schemars`, as Omnix's `ops-schema`).

### 7. Where it lives

A Cargo workspace in this repo: `crates/sheet` (the core, a library),
the studio binary beside it. Omnix depends on the core by path
(`../rushix/crates/sheet`) and its `notes.rs` goes away. A path
across two repos builds locally; if either is ever published, the
dependency becomes a git or crates.io one.

## The plan, a local commit per step

1. **Workspace and model.** `crates/sheet` with the typed model, the
   unknown kept, `Time`'s integers, load and save. Tests: a synthetic
   sheet read and written byte for byte; every shape of mark; unknown
   fields at each level kept. A one-off check on a scratch copy of Kos's
   file (never committed): identical after a round trip.
2. **Ids.** Given on load, kept, unique; the web sheet's saves keep them
   (checked with footage-agent's page against a scratch copy).
3. **Operations and `check`.** Each op with its tests; `check` after
   every op.
4. **Undo, and properties.** `Edit` with `begin` / `commit`; proptest:
   random op sequences keep `check`, and undo restores each state.
5. **`Store`.** Fresh read, apply, check, atomic write, `.bak`; a test of
   two writers (an op applied over a file changed meanwhile).
6. **CLI and schema.** `apply`, `describe`, `schema`; tests of each.
7. **Omnix on the core.** `notes.rs` replaced; the bin player's I / O,
   role, note and delete as ops by id; its 176 tests and the hand checks
   still pass. The web sheet's server untouched.
8. **Docs.** This page becomes the core's reference; both repos'
   contexts and the roadmap updated.

Then the studio's window (the next plan), on this core.

## Decided (user, 2026-11-04)

1. **The name:** Rushix; the binary `rushix`, the core `rushix-sheet`
   (`crates/sheet`).
2. **Ids in the real sheet:** yes, written.
3. **Time precision:** what the core is given, to a thousandth; the web
   sheet's tenths stay as they are.
