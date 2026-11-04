# Rushix

Bringing footage in and choosing it. *Rushes* is the trade's word for raw
footage before the edit: Rushix takes it off cards and phones, lets you
look through it and mark what's worth keeping, files the originals away,
and hands the chosen material to an editor (Omnix,
`../omnix-360-studio`). Rust, built the way Omnix is.

Status: the contact sheet's core and a command line work; the window is
being built. It replaces footage-agent's web contact sheet step by step.

## Three places

Rushix works across three places, each set in its settings (nothing about
them is fixed in the code):

```text
TMP        where new material lands first, straight off a card or a phone,
           checked, before anything is decided about it
rushix     the projects: a folder per trip or story, with the clips chosen
           for it, its marking and its edits
sources    every original, kept as it came off the camera, never changed,
           filed by trip and day; one copy now, two some day
```

**Sources can live anywhere a folder can:** a local disk, a removable
drive, a network share (mounted as a folder). A cloud bucket is planned:
the setting names a kind and a place, so a bucket becomes one more kind
rather than a change everywhere.

## What it does, in order

1. **Bring in.** Media from a card, a phone or a folder are copied into
   TMP and checked against the source (sizes, then hashes). The card is
   left as it was.
2. **Choose.** The contact sheet shows a day's clips as tiles: rate each
   (✓ take, ? think, ✕ junk), note it, mark segments (in and out, a role
   such as *B-roll* or *talk*, a note), flag what needs finding (a meme, a
   sound), and write the day's story.
3. **File away.** The originals move from TMP into sources, by trip and
   day, unchanged.
4. **Hand over.** The chosen clips go into a project in `rushix`, and an
   Omnix edit is made from the marking (Omnix reads the marking itself
   too: segments set in its bin land in the same file).

## A project folder

The numbered folders are for people, opened in a file manager; everything
a program keeps lives in `rushix/` beside them.

```text
<project>/
  01_footage/<day>/<clip>    the clips chosen for this project (copies)
  02_audio/music, 02_audio/voiceover
  03_proxies/<day>/<clip>    the cameras' own light copies (DJI's LRF)
  04_project                 edits: .json for Omnix, .mlt for Kdenlive
  05_export                  finished videos
  06_external                outside video: memes, inserts, transitions
  07_sfx                     sound effects
  08_graphics                pictures over the video
  09_luts                    colour LUTs (.cube)
  rushix/
    project.json             what the project is, where it came from
    sheet.json               the marking: days, ratings, notes, segments
    analysis/<day>/<clip>.footage.json
    thumbs/<day>/<clip>/0001.jpg …
    voice/<day>/<clip>/<recipe>.flac   cleaned voices
```

A clip is named in the marking by its path under `01_footage`:
`<day>/<file>`.

## Command line

```text
rushix                         the window (the last project, or pick one)
rushix sheet apply <sheet> <ops.json|->   change a marking: checked, all or none
rushix sheet check <sheet>     what's wrong with a marking
rushix sheet describe <sheet>  a marking as text, a line per clip and segment
rushix sheet schema [ops|sheet]
rushix install                 the icon and a desktop entry (Linux)
```

The marking's format and the ops that change it: `docs/SHEET-CORE.md`.
Where the project stands and what's next: `docs/CONTEXT.md`.

## Building

```text
cargo build --release
cargo test
```

## Licence

MIT OR Apache-2.0, at your option (`LICENSE-MIT`, `LICENSE-APACHE`).
