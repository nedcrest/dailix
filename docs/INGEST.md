# Bringing footage in: from a card to the archive

Design for the user's approval before code (2026-11-05), as
`SHEET-CORE.md` was. The first layer of the studio (README, "What it
does": bring in, choose, file away, hand over), shaped by the UI
research (`UI-RESEARCH.md` 1, 3.3, 4) and the user's words: "the global
studio makes it easy to move files from different media into a TMP
place, then the needed things are chosen in the contact sheet, and
after that they move from TMP into the archive (categorised: a disk or
disks, and buckets with the sources; one copy for now, two some day)."

## What there is to learn from (footage-agent's `ingest.py`, Omnix gap 34)

Kept, because they were right:
- a camera's volumes are found by a `DCIM` folder at their root;
- a camera writing to its internal memory and its card at once (the DJI
  Action 2) is merged into one day, sorted by when each clip was filmed,
  not by which volume it was on;
- originals on the card are never touched, never deleted: freeing the
  card is the user's act, after Dailix says it's safe;
- a dry run first (here: the plan is shown before anything is copied).

Missing there, needed here (gap 34's list):
- phones: Android mounts over MTP under `/run/user/<uid>/gvfs/mtp:*`,
  not `/media`;
- the drone (DJI Neo 2) and Osmo 360 cards, Canon R50's, a lavalier's
  WAVs: recognised and named;
- files already brought in skipped, by content;
- the device named per clip;
- progress, and a clear end.

Left behind on purpose (don't inherit, improve): taking "today's files" by
the file's modified time (a phone's copy has the copy's time), and
writing straight into a project folder.

## The three places, and what moves between them

```text
card / phone / folder ──copy, verified──▶ TMP ──move, verified──▶ sources
                                           │                        │
                                           └── the contact sheet ───┘ reads either
                                                  │
                                                  └── chosen clips ──copy──▶ project (01_footage)
```

- **TMP** (`DAILIX-TMP`): new material, sorted by day, waiting to be
  chosen and filed. Nothing in it is the only copy for long: the card
  stays as it was until TMP's copy is verified.
- **sources** (`VIDEO-ARCHIVE`): every original as it came off the
  camera, by trip and day, never changed. One copy now; a second place
  (another disk, a bucket) later, each day counting its copies.
- **projects** (`DAILIX`): a trip's project, the clips chosen for it
  copied into `01_footage`, its marking, its analysis.

## The flow, step by step

1. **Sources found.** Mounted volumes with `DCIM` (cameras, drones),
   MTP phones under gvfs, and any folder the user picks (a Mimo dump, a
   downloads folder). Each is remembered by its volume's UUID (or, for a
   folder, its path) with the name the user gives it ("Action 2 card",
   "phone"), so the next time it's known.
2. **Scanned.** Every media file: video (MP4, MOV, OSV, INSV), photo
   (JPG, HEIC, DNG, CR3), sound (WAV from a lavalier or recorder), and
   the cameras' sidecars (DJI's LRF proxies, THM thumbnails, SRT
   telemetry). For each: its size, when it was filmed, which device.
   - *When filmed*: the container's `creation_time` (ffprobe), else a
     name with a date in it (`DJI_20260821_102204…`, `20260818_202527`),
     else the file's modified time, marked as a guess.
   - *Which device*: the file's own tags (encoder, make and model),
     else the source's remembered name.
   - *Which day*: the date where it was filmed. Cameras write UTC or the
     camera's clock; a trip has a time zone (Kos: UTC+3), so a clip at
     23:30 local stays on its day.
3. **Known already?** A clip's content is fingerprinted (below); one
   already in TMP or in the sources is shown dimmed, "in the archive
   since 2026-08-26", and isn't copied again, whatever its name or date
   (phones rename and re-date).
4. **The plan shown** (the ingest view, UI-RESEARCH 6.2): by day, new
   against known, how many GB, where each goes. The user unticks what
   they don't want (a day, a clip), picks the trip (or "no trip yet").
5. **Copied to TMP, verified.** Read once from the card while computing
   its hash, written to TMP under a hidden name, synced, read back and
   hashed again, renamed when they match (as the Kos import, now with
   hashes). A day's checksums go into a manifest file beside it. Many
   sources copy at once, each its own job with its progress; one
   failing stops only itself.
6. **The end, said plainly**: "Action 2 card: 143 files verified in TMP
   — safe to format", or what failed and why. Dailix never formats or
   deletes anything on a card or a phone.
7. **Sorted into trips** (Omnix gap 117, built 2026-11-16): the TMP
   sorter (home's «TMP · N new», or `dailix tmp`) shows TMP's videos by
   the day they were filmed; the picked are filed into a trip (an
   existing one or a new one) and into its parts, named lists of its
   clips across days («Rides on Kos»). Filing copies them into the
   trip's `01_footage/<day>/` (`add::add`); TMP keeps them, its ledger
   (`.dailix-filed.json`) saying where each went, for step 8.
   *Built before steps 1–6: today what's in TMP was put there by hand.*
8. **Chosen** in the contact sheet, on the trip's days: as Kos is
   now, ratings, segments, the day's story; a part's clips alone if
   asked.
9. **Filed away**: a day's originals move from TMP to
   `sources/<trip>/<day>/`, each hash checked at the destination before
   TMP's copy goes (on the same disk it's a rename, instant). The day's
   manifest goes with them; `MANIFEST.md` for the trip is written or
   added to, as Kos's archive has one.
10. **Handed over**: the chosen clips are copied into the trip's project
   (`01_footage/<day>/`), the marking already there.
11. **TMP freed** only for days whose sources copy is verified; it shows
    what goes and what stays.

## Fingerprints, catalogue, manifests

- **Hash**: BLAKE3 over the whole file (fast, Rust, no vendor SDK). For
  a quick "known already?" before reading a whole 2 GB clip: size plus
  a hash of its first, middle and last MiB finds candidates; a full
  hash settles it.
- **Catalogue**: each place keeps one, `<place>/.dailix/catalog.json`:
  hash → where (trip, day, file), size, when filmed, when it came in. It
  is what "known already?" asks, and what offline places answer from (a
  drive unplugged is still known: UI-RESEARCH 4.1). It can be rebuilt by
  hashing the place again.
- **Manifest** per day: `<day>/.dailix/manifest.json`, the files and
  their hashes, from the copy that made them; "re-verify" hashes the day
  again against it and names any file that changed (silent damage).
  Also written as a plain `checksums.b3` (hash, two spaces, name), so
  other tools (`b3sum -c`) can check it without Dailix.

## The ingest view (UI-RESEARCH 6.2)

- Left: the sources found (● present, ○ remembered but absent), with
  their names; "+ Folder…".
- Right: the plan by day: a row of small tiles per day (dimmed: known),
  new / known counts, GB; a trip picker with "new trip…" (name, dates
  suggested from the days).
- Below: the jobs, each with copy and verify progress and speed, and
  its end line ("safe to format").
- The home view's places strip gets "N days only in TMP" (not yet in
  the sources): UI-RESEARCH 3.4.

## The core and its tests, as the rest of Dailix

A module per part, each tested, the window only calling them; commands
for the tests and an AI:
- `scan.rs`: sources found, files read (time, device, day);
- `catalog.rs`: hashes, the catalogue, "known already?";
- `copy.rs`: a verified copy (read, hash, write, sync, read back,
  compare, rename), resumable;
- `ingest.rs`: the plan and its jobs;
- `file_away.rs`: TMP to sources, verified;
- CLI: `dailix ingest plan <source>…` (prints the plan), `dailix ingest
  run …`, `dailix verify <place|day>`, `dailix file-away <trip> <day>`.
- Tests on made-up sources in temp folders: a card with DCIM and LRF
  and THM; the same clip renamed (found by content); a copy cut off and
  run again; a destination corrupted after the copy (re-verify names
  it); a day across midnight in another time zone.

## Steps, a local commit each

1. `scan.rs`: sources and files (time, device, day), `ingest plan`
   printing it; tests.
2. `catalog.rs` and hashing; "known already?"; a catalogue built for
   the existing sources (Kos's archive) by hashing it once.
3. `copy.rs`: the verified copy, its manifest; tests with corruption.
4. The ingest view: sources, plan, jobs, the end line.
5. A trip in TMP opens in the contact sheet (sidecars made at ingest:
   at least the length, so the sheet can list the clips; the analysis
   passes move to Dailix later).
6. Filing away and freeing TMP; the home view's "only in TMP".
7. Docs.

## Decided (user, 2026-11-05)

1. A day goes to the sources right after its copy to TMP is verified;
   choosing comes after, from either place.
2. A project gets only the chosen clips (✓ and ?, and any with
   segments); the sources keep everything.
3. Photos come in too, into the sources; not shown in the sheet yet.
4. A trip has a time zone, suggested from the phone's clips (they carry
   the local offset) where it can be.
5. A second copy later; the catalogue and manifests are made ready for
   it now.

Implementation deferred by the user (2026-11-05): Kos is marked and its
edit goes on in Omnix; this design waits for its turn. Still waiting
(2026-11-09): its first step, one's own videos into a project (Omnix
roadmap «Dailix: свої відео в проєкт», gaps 74 with 80), comes after
Omnix's milestone «Ефекти, колір і розумний звук». The chosen clips
land in a trip whose videos sit in `videos/<name>/` (gap 82).
