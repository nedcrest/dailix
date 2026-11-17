# Changelog

## 0.1.0 — Dailix's first release (2026-11-17)

Dailix is the contact sheet before the edit: footage from the cards
sorted into trips, the originals kept safe in an archive, a trip looked
through and its best moments chosen, then handed to
[Omnix](https://github.com/nedcrest/omnix-360-studio) as a first edit.
Built with Claude, as Omnix is ([how](https://github.com/nedcrest/omnix-360-studio/blob/main/docs/AI-DRIVEN.md)).
The guide: [Bring the footage in](https://github.com/nedcrest/omnix-360-studio/blob/main/docs/guide/tutorial-0-bring-in.md),
then [Choose in Dailix](https://github.com/nedcrest/omnix-360-studio/blob/main/docs/guide/tutorial-1-dailix.md).

### Bringing footage in

- **The TMP sorter**: what came off the cards and phones, as tiles by
  the day each was filmed; picked by click, Ctrl, Shift or the arrows;
  filed into a trip (or a new one, named by the month it was filmed)
  under its day, and into the trip's **parts** — your own groups of
  clips across days.
- **The archive**: a day's, or all of TMP's, originals copied into
  Sources by trip and day (those in no trip yet under
  `_not-in-a-trip`), each copy checked by its BLAKE3 hash; only then
  does TMP let them go, to the trash. «Remove picked from TMP», asked
  first. A bar says how far a job is.
- **How Dailix works**: the whole way in seven steps, from ☰.
- `dailix tmp list | file | archive | free`.

### Choosing

- **Trips** as cards; a trip's page with its edits, parts and days.
- **The contact sheet**: clips as tiles by day, rated with P, ?, X;
  segments marked with I and O, each with a role and a note; flags and
  moments; each day's story; a filter by rating and by part; a group of
  clips put into a part at once.
- **Cleaned voices**: a clip's sound cleaned (DeepFilterNet, if
  installed), heard against the original at one loudness, the chosen
  one used in the edit.
- **→ Omnix**: an edit of a day, the trip or a part, from the marking,
  opened in Omnix.

### Underneath

- The marking is a plain JSON file in the trip, changed only through
  checked operations (`dailix sheet apply | check | describe | schema`),
  the same ones Omnix's bin uses.
- One folder per trip, in a fixed layout both programs read.
