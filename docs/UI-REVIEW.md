# Dailix's interface, looked at again (2026-11-14)

The user: Dailix isn't friendly enough; better panels, button groups,
«as the best apps do». Two reviews: every screen and control in the code
(where it is, its style, what it does), and how the culling and media
apps and the platforms' design guides lay these jobs out (Lightroom
Classic, Photo Mechanic, darktable, digiKam, Adobe Bridge, DaVinci
Resolve's Cut page, Final Cut Pro's browser, Kyno, Frame.io; GNOME HIG,
Microsoft Fluent, Apple HIG, Material 3; sources in the session). This
page is the proposal, for the user's choice before any code.

## What's wrong today

- **One kind of action, many looks.** Buttons left without a style draw
  as primary (the accent) by accident: «Open…» is the loudest thing in
  the top bar and the least used; the Keys window's «Close», the story's
  «Save», «+ New trip», Settings' «Choose…», ▶/⏸ and the voice's «Make»
  too. Secondary buttons come in four sizes, danger ones in four, modals
  with four paddings. No style module: every size and colour is written
  where it's used.
- **The same action in several places, said differently.** A day's
  «→ Omnix» / «Empty day» sit on the day card and again in the contact
  sheet's day heading (tooltips worded apart); a place's folder is chosen
  four ways («Change…», «Choose…», «Choose the X folder…», a chip);
  places are named and ordered three ways (chips TMP, projects,
  sources; welcome Trips, TMP, Sources; Settings projects, sources,
  TMP); «Empty day» vs «Empty edit».
- **The danger beside the everyday.** «Delete trip…» in red in the same
  row as «→ Omnix», «Empty edit», «+ Add videos…»; a red button under
  every edit card. The guides: one accent or destructive button per view;
  deleting goes in a menu.
- **Ratings as three grey buttons on every tile**, the rating not shown
  as a colour on the tile; nothing filters by rating. Mouse and keys
  differ (a click toggles, a key sets and moves on); «unrated» has no
  button.
- **The player pane**: a large empty box until a clip plays, the clip's
  facts squeezed at the bottom, the voice row two lines of 12 px controls
  under no heading; making marks is keyboard only (I, O, M, N).
- **No way back to the trip** from the contact sheet: ⌂ Home goes to the
  trips list.
- **Long jobs without a word**: adding videos (no progress, success
  silent), «→ Omnix» (runs on the window's thread), the delete question
  (sizes counted on the window's thread: a big trip freezes it), voices.

## Bugs found on the way (fix now, whatever the design)

1. **A day's story typed and not saved is lost** on going Home or
   opening another trip, without a word.
2. **The «→ Omnix» question can't fit two edit cards side by side**
   (484 px inside, 486 needed): they stack one a row.
3. **The delete question freezes the window** while it counts a big
   trip's files (on the window's thread); the trash itself too.
4. **Adding videos says nothing** when it's done.

## How the best apps do it (what to take)

- **Lightroom Classic**: sources left, grid centre, inspector right; the
  rating by keys (P pick, X reject, U clear), shown on the thumbnail
  (rejects dimmed); a slim filter bar above the grid (by flag, stars).
- **Photo Mechanic**: one key per decision, then on to the next clip.
- **digiKam**: three-state pick (rejected / pending / accepted), red /
  yellow / green: Dailix's ✕ ? ✓ exactly.
- **Final Cut Pro**: favourite / reject as green / red lines on the
  item; a filter «All, Hide rejected, Favorites, Rejected».
- **Resolve's Cut page**: a whole shoot as one tape; one viewer.
- **Frame.io**: notes anchored to a time, listed beside the player; a
  click jumps there (Dailix's marks list).
- **GNOME / Apple / Fluent**: a header with few controls (back and the
  place you're in at the start, one primary action and the main menu ☰ at
  the end); app-wide things (zoom, keys, settings, open) in the main
  menu; one accent button per view; destructive actions in a ⋯ menu,
  last, apart; a linked (segmented) control for one-of-several state;
  everything frequent has a key; a command on many pages stays in the
  same place.

## The proposal, screen by screen

### 0. Foundations (first, small, everything else stands on it)

- **One style module** (`ui.rs`): the button kinds (primary, secondary,
  quiet, danger) at one size each, the modal's padding, the card and
  panel looks, the spacing steps; every screen uses it.
- **One set of words**: the places «Trips, TMP, Sources» in that order
  everywhere; «Choose…» everywhere; «Empty edit» everywhere (a day's or
  the trip's).

### 1. The top bar, the same on every page

```
[‹] Tutorial › 22 Aug                                   [Open in Omnix ▾] [☰]
```
- Back and where you are: Home › trip › day (a click goes there); no
  ⌂ on Home itself. The window titled by the trip, not «Dailix».
- ☰ main menu: text size (− 100% +), Open…, Keyboard shortcuts (F1),
  Settings (Ctrl+,), About.
- At most one accent button, the page's main action.

### 2. Home

```
Trips                                                   [+ New trip]
[card] [card] [card] …
──────────────────────────────────────────────────────────────────
TMP 231 GB free · Trips 231 GB free · Sources 231 GB free   (a quiet footer)
```
- «+ New trip» the one accent button, by the heading.
- The places as a quiet line at the bottom (status, not the main thing),
  louder only when a disk is low or away; a click still chooses.
- A ⋯ on each card: Open, Rename, Delete trip….

### 3. A trip's page

```
Tutorial                                 [→ Omnix ▾]  [Open the sheet]  [⋯]
18–24 Aug 2026 · 4 days · 6 clips · 0 rated
Description  […………………………………………]
Tags         [tutorial] [guide] [+]
── Edits ─────────────────────────
── Days ──────────────────────────  (a whole card opens its day)
```
- **«→ Omnix ▾»**, one split button: the click makes an edit of every
  day from the marking; its menu: Empty edit, the days one by one.
- **⋯**: Add videos…, Rename, Show in the file manager, — Delete trip….
  No red button on the page.
- Labels on the fields; tags as chips.
- Day cards: the whole card opens the day; its own «→ Omnix ▾» on the
  card (the same split button, smaller), no second row of buttons.
- An edit card's ⋯ (Open in Omnix, Move to trash…) instead of a red
  button under each.

### 4. The contact sheet

```
┌ Days ──────┐┌ [All 6│Unrated 6│✓ 0│? 0│✕ 0]   size ─○─                ┐┌ Clip ──────────┐
│ 18 Aug 1 ▮ ││ 22 Aug · 2 clips · 0 rated              [→ Omnix ▾] [⋯] ││ DJI_0021 · 0:56│
│▸22 Aug 2 ▯ ││ the day's story…                                        ││ [ player ]     │
│ 23 Aug 2 ▯ ││ [tile] [tile]                                           ││ strip, marks   │
│ 24 Aug 1 ▯ ││ 23 Aug … (the heading stays at the top as you scroll)   ││ ▸ Marks (3)    │
└────────────┘└─────────────────────────────────────────────────────────┘│ ▸ Voice        │
                                                                         │ ▸ About the file│
                                                                         └────────────────┘
```
- **Days on the left**: a slim list with each day's rated share,
  following the scroll, a click jumps; it folds away. The days stay one
  long sheet (the whole trip as one tape).
- **A filter bar** over the tiles: All, Unrated, ✓, ?, ✕ with their
  counts — «show me what I haven't rated». A tile size slider.
- **A tile shows its rating as a colour** (a green ✓, amber ?, red ✕ line
  or corner; a rejected tile dimmed); the three buttons come as one
  linked ✓│?│✕ only on the tile under the pointer or picked. Keys stay:
  P, /, X, U.
- **The day's heading** once, sticky, with «→ Omnix ▾» and ⋯ (the day
  card's buttons gone from the trip page's second place).
- The story saved by itself as it's typed (no Save to forget).

### 5. The player pane (an inspector)

- The player at the top, the strip under it a little taller, the marks
  in it as coloured ranges.
- Buttons for what's keyboard only now: In, Out, Flag, Moment (I O M N
  as their tooltips).
- Folding sections with headings: **Marks** (a click jumps), **Voice**
  (a heading; «Make» the one accent there; the recipe labelled
  «Clean-up»), **About the file**.
- No empty box: with no clip picked, a short line says what to do.

### 6. Feedback

- A small busy line in the top bar for jobs under way (adding videos,
  making an edit, voices, posters), what's done when it ends.
- The error line dismissable, and only errors in it.

## In what order (the most for the least first)

1. Bugs 1–4 above. *Done.*
2. Foundations (one style module, one set of words) — nothing looks
   new, everything after is quicker. *Done* (`ui.rs`, `PlaceKind::label`,
   `home::to_omnix`).
3. The top bar with ☰ and the way back. *Done.*
4. Ratings: the colour on the tile, the linked toggle, the filter bar.
   *Done.*
5. The trip page: «→ Omnix ▾» and ⋯; no red in the row; labels.
   *Done* (tags still typed, not chips).
6. The player pane as an inspector, with buttons for marks. *Done.*
7. The days list and sticky headings. *Done* but the sticky heading
   (iced has no sticky layout; the bar's path names the day), and the
   list lights the picked clip's day rather than following the scroll.
8. Home: «+ New trip» as the accent, the places as a footer, a ⋯ per card.
   *Done.*
9. Feedback for long jobs. *Done.*

The welcome stays as it is (the user approved it, 2026-11-14).
