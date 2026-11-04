# UI research: ingest, culling, library, archive

What the best tools in each step do, and what Dailix should take.
Written 2026-11-05. Sources are linked. Lines marked **(knowledge)** come from
general familiarity with the product, not from a page read for this document —
check before relying on exact names or shortcuts.

Dailix's four steps, for reference: **bring in → choose → file → hand off.**
The overall goal is to make the file manager unnecessary.

---

## 1. Ingest / offload

### 1.1 Source → destinations as one "job" card
- **What:** one source (card, phone, folder) on the left, one or more destinations
  on the right. Each pairing is a job with its own progress bar. Several jobs run at once.
- **Who:** Hedge OffShoot (drag a card onto destinations, **(knowledge)** for the layout);
  DaVinci Resolve Clone tool: "Add Job", drop a source in "Drop source here", add
  several destinations, and a checksum report is written to each destination
  ([Resolve ingest how-to](https://jayaretv.com/tips/media-ingest-with-davinci-resolve/),
  [Resolve manual](https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part482.htm)).
- **Dailix:** model an ingest as a card `source → TMP (→ archive copy 2 later)`, with
  per-destination state. Auto-detect sources when a card is mounted.

### 1.2 Verification modes, named plainly
- **What:** OffShoot has three modes. *Transfer* compares sizes only. *Source* reads
  the source independently to rule out read errors. *Source & Destination* reads
  everything back and compares checksums, at about 2× the time. XXH64 is always
  computed. ASC MHL (a checksum list file) is written, and the MHL is re-checked when
  the disk is idle ([OffShoot verification](https://docs.hedge.video/offshoot/features/verification)).
  Silverstack compares each file with its copies byte by byte and shows the
  verification state per file, clip and bin
  ([Silverstack offload](https://kb.pomfort.com/silverstack/offloadandbackup/),
  [features](https://pomfort.com/silverstack/features)).
- **Dailix:** always use full read-back verification (xxh3/xxh64) for TMP and archive
  copies. Show a single green "verified" badge per clip and per day. Write a
  checksum manifest next to the files so another tool can re-verify them.

### 1.3 "Already imported" / duplicate skip
- **What:** OffShoot matches duplicates on folder structure, name, size, creation
  and modification date, then skips them. Resume skips what is already done and
  re-copies only partial files
  ([verification](https://docs.hedge.video/offshoot/features/verification),
  [stop & resume](https://docs.hedge.video/offshoot/features/stop-and-resume),
  [copy just what's new](https://hedge.co/blog/save-time-copy-just-what-s-new)).
  Lightroom's import treats original name + EXIF capture time + size as a duplicate,
  and **greys out** those thumbnails in the import grid
  ([Adobe help](https://helpx.adobe.com/ie/lightroom/help/photo-video-import-options.html),
  [forum thread](https://community.adobe.com/t5/lightroom-classic-discussions/duplicate-lightroom-imports/m-p/9006705)).
  Forums report false matches with phones and changed modification dates.
- **Dailix:** detect duplicates by content hash (plus size), not by name. A phone
  renames files and changes timestamps, and name matching breaks there. In the
  source grid, show what is already in TMP or the archive as dimmed with a "✓ in
  archive / day 3" badge, and import only what is new by default.

### 1.4 Pick before you copy
- **What:** Photo Mechanic can open a card as a contact sheet so you choose, then
  "Ingest From Selection"
  ([Fstoppers PM review](https://fstoppers.com/originals/photo-mechanic-plus-lightroom-killer-depth-fstoppers-review-part-1-importing-525089),
  [jmpeltier](https://www.jmpeltier.com/importing-editing-metadata-photo-mechanic/)).
  Lightroom's import window has a thumbnail grid with checkboxes plus a file-handling
  panel on the right **(knowledge)**.
- **Dailix:** default to "everything new on the card" (cards are small and choosing
  comes later), but allow unticking whole days or obvious junk in the source grid.

### 1.5 Naming / folder templates
- **What:** Lightroom: "Into subfolder", "By date" with a date-format choice, plus a
  rename template ([Adobe help](https://helpx.adobe.com/ie/lightroom/help/photo-video-import-options.html)).
  Photo Mechanic has a variable-based path and rename ("{year}/{month}…") **(knowledge)**.
  Silverstack and OffShoot save settings as reusable presets/templates
  ([Pomfort cascading copy](https://kb.pomfort.com/?p=4378)).
- **Dailix:** one fixed, visible template `trip/YYYY-MM-DD/<device>/<original name>`
  with a live preview of the resulting path. Keep original filenames (relinking and
  camera sidecars depend on them). Do not offer a token editor until someone asks.

### 1.6 Progress, logs and "safe to eject"
- **What:** OffShoot shows status under each progress bar, and clicking it opens the
  verification log. Silverstack shows a persistent job status and a status per
  volume, and keeps a log of all copy activity
  ([Silverstack main screen](https://kb.pomfort.com/silverstack/primary-features-sst-xt/the-silverstack-main-screen/)).
- **Dailix:** per job show files, bytes, speed, ETA and the current phase (copy /
  verify). At the end show one clear state: **"Card X: 412 files verified in TMP —
  safe to format"** or a red list of failures. Keep a log per ingest.

### 1.7 Device importers (DJI, GoPro)
- **What:** DJI Studio and GoPro Quik import from a connected camera, then group by
  date. They are simple but hide where the files go **(knowledge)**.
- **Dailix:** learn camera folder layouts (DCIM/DJI_xxx, .LRF proxies, .LRV/.THM,
  .osv for 360) and treat proxy and sidecar files as part of the clip, never as
  separate tiles.

---

## 2. Culling / logging / selection

### 2.1 Keyboard-first rating with auto-advance
- **What:** Photo Mechanic: T toggles the tag, 1–8 set colour classes, number keys
  can be mapped to stars instead, and **auto-advance** jumps to the next item after
  a rating, tag or colour change
  ([PM101 notes](https://home.camerabits.com/wp-content/uploads/2023/06/Photo_Mechanic_101_Notes.pdf),
  [colour classes](https://docs.camerabits.com/support/solutions/articles/48001142942)).
  Lightroom: P = pick, X = reject, U = unflag, 0–5 stars, 6–9 colour labels, with
  Caps Lock to auto-advance **(knowledge)**. FCP: F = favourite, Delete = reject,
  U = unrate
  ([PVC rating & filtering](https://www.provideocoalition.com/rating-and-filtering-in-final-cut-pro-x/)).
- **Dailix:** three keys for take / think / junk (e.g. **P / ? / X**, mirroring
  Lightroom), with auto-advance on by default. Hands never leave the keyboard.

### 2.2 Range marking on the clip (subclip/segment)
- **What:** FCP marks a range in the skimmer, F makes it a favourite range, and
  ⌘K makes it a keyword range. Each keyword becomes a **Keyword Collection**
  automatically. Rejecting a middle range splits the visible clip in two
  ([Larry Jordan](https://larryjordan.com/articles/fcp-x-create-subclips-with-keywords/),
  [forum](https://creativecow.net/?p=2439224)).
  Kyno: in/out points and markers in the player become subclips. Version 1.3 can
  export them as Premiere markers or subclips
  ([Larry Jordan on Kyno 1.3](https://larryjordan.com/blog/news-kyno-1-3/),
  [PremiereBro](https://premierebro.squarespace.com/blog/10-ways-kyno-can-speed-up-your-pre-editing-premiere-pro-workflow)).
  Premiere: I/O in the Source monitor, then "Make Subclip" **(knowledge)**.
- **Dailix:** I / O on the tile's skimmer or in the player, then one key for the role
  (B = B-roll, T = talk, C = cutaway, R = reaction) and Enter for a note. Draw the
  ranges as coloured bars on the tile's filmstrip, and make every role a smart
  collection automatically, as FCP does with keywords.

### 2.3 Skimming the tile
- **What:** FCP's skimmer scrubs a filmstrip under the mouse. Kyno and Lightroom
  show hover scrub on video thumbnails **(knowledge)**.
- **Dailix:** hover-scrub on every tile using the camera proxy (.LRF/.LRV) or a
  generated filmstrip, with J/K/L and Space in the large viewer.

### 2.4 One long "source tape" for a day
- **What:** Resolve's Cut page Source Tape plays every clip in the bin as one long
  tape. You scrub through it and mark in and out as you go
  ([OWC blog](https://owc.com/blog/how-to-edit-b-roll-and-stock-faster-with-davinci-resolves-cut-page),
  [Cut page manual](https://ltbits.github.io/davinci-resolve-manuals/DR18/DR18-RM04-TheCutPage.pdf)).
- **Dailix:** a "play the day" mode where clips play back to back, filtered (e.g.
  only "take" + "think"). I/O and role keys work the same as in the grid. This is the
  fastest way to write the day's story.

### 2.5 Filters by rating/role, saved as smart collections
- **What:** Lightroom's filter bar (attribute / text / metadata) and smart
  collections **(knowledge)**. Photo Mechanic's colour-class widget filters the
  sheet. Kyno filters by rating, tag and duration
  ([Kyno review](https://www.newsshooter.com/2016/09/15/kyno-the-footage-screening-logging-organising-and-transcoding-program-for-filmmakers-in-now-available/)).
- **Dailix:** a single filter row above the sheet with chips: rating, role,
  "flagged for later", device, has-note, duration. A filter can be saved as a smart
  collection that spans days.

### 2.6 Compare / survey
- **What:** Lightroom Compare (C) shows two images side by side, and Survey (N)
  narrows a group by removing the losers **(knowledge)**. Aftershoot and FilterPixel
  group similar shots and flag closed eyes and blur in buckets
  ([Aftershoot review](https://digitalcameraworld.com/reviews/aftershoot-review),
  [Imagen comparison](https://imagen-ai.com/post/best-ai-culling-tools/)).
- **Dailix:** later. A "survey" of 2–4 selected clips playing in sync is useful for
  multi-camera moments. Automatic similarity grouping can wait (it is a nice-to-have).

### 2.7 Notes and markers visible on the tile
- **What:** Avid bins have Frame, Script and List views. Script view puts a frame
  next to a free-text log **(knowledge)**. Frame.io shows comment markers on the
  scrub bar **(knowledge)**.
- **Dailix:** each tile shows the rating badge, role-coloured range bars, a 🚩 icon
  for "find later", and the first line of the note. Add a list view (like Avid's
  Script view) for writing many notes quickly.

### 2.8 The day's story
- **What:** no tool here does this well. The nearest are Avid Script view and Kyno's
  description field **(knowledge)**.
- **Dailix:** a story pane beside the sheet: a free-text editor where you can drop a
  clip reference (`@clip 14:32 talk`) that links back to the tile. This is the
  distinctive feature.

---

## 3. Library visualisation instead of a file manager

### 3.1 Trips, then days, as the main axis
- **What:** Apple Photos builds "Trips" collections automatically from location and
  date ([Apple guide](https://support.apple.com/guide/photos/phtacde28864/mac),
  [forum on how trips are decided](https://discussions.apple.com/thread/255713589)).
  Immich and Google Photos have a timeline with a date scrubber on the right edge,
  and Immich's map clusters open a mini-timeline
  ([Immich 2.6](https://pixelunion.eu/blog/2026/03/immich-v2-6-0/)).
- **Dailix:** the home view is a list of trips. Each trip is a row of day cards with a
  hero thumbnail, the number of clips, the share rated, and the storage state. Trips
  are created by the user (with the date range suggested from capture times), not
  guessed by AI.

### 3.2 Folders panel vs collections
- **What:** Lightroom keeps a physical Folders panel and virtual Collections side by
  side **(knowledge)**. Kyno treats folders as the library, with a "Drilldown" mode
  that shows all media under a folder, and it remembers workspace folders
  ([cre-8 review](https://cre-8.com.au/?p=2573),
  [RedShark](https://www.redsharknews.com/post-vfx/item/3940-new-kyno-media-screening-tool-on-test)).
  digiKam and Darktable do the same with albums or film rolls on disk **(knowledge)**.
- **Dailix:** hide folders by default. Trip, day and collections are the navigation.
  Offer a "Show in folder" action only as an escape hatch. The archive's folder
  layout mirrors trip/day, so the two never disagree.

### 3.3 Where is the original? (status per clip and per day)
- **What:** Silverstack lists every location of a clip ("resources") with its
  verification state, and has a Volumes panel with a playback priority
  ([Pomfort article](https://pomfort.com/article/one-clip-on-multiple-volumes-with-silverstack/)).
- **Dailix:** each clip has a status chip strip: `TMP · ARCHIVE(1/2) · PROJECT`, with
  the drive name. Roll it up at day and trip level ("Day 3: 100% archived, 1 copy").

### 3.4 Storage overview
- **What:** Resolve's Media Storage panel lists mounted volumes **(knowledge)**.
  OffShoot and Silverstack show status per volume (sources above). Eagle and Plex
  show library sources as a list **(knowledge)**.
- **Dailix:** a storage strip on the home view: each storage target (TMP, archive
  disk A, NAS, later a cloud bucket) with free space, online/offline status, and
  "N days only here" warnings.

---

## 4. Archive / storage management

### 4.1 Offline volumes stay browsable
- **What:** Lightroom keeps showing photos on an unplugged drive using cached
  previews, greys out the volume, and marks missing folders with "?". A moved folder
  is fixed with "Find Missing Folder" on its parent, which relinks everything below it
  ([Lightroom Killer Tips](https://lightroomkillertips.com/when-photos-go-missing/),
  [Adobe forum](https://community.adobe.com/t5/lightroom-classic-discussions/how-to-fix-folder-is-missing-please-re-link-errors-in-your-image-catalog/m-p/15618647)).
  Canister keeps a catalog of each LTO tape so it can be searched without the tape,
  shown as a virtual "LTO Archive" drive
  ([Canister catalogs](https://docs.hedge.video/canister/features/catalogs)).
- **Dailix:** identify volumes by a UUID or a marker file, not by mount path. When a
  drive is unplugged, keep its tiles, proxies, ratings and notes usable with an
  "offline: Archive-A" badge. Relink automatically when it comes back, and support
  relinking a whole tree at once.

### 4.2 Two-copy status
- **What:** Silverstack's "one clip on many volumes" model, with backups run from the
  library to new destinations ([Pomfort](https://pomfort.com/article/one-clip-on-multiple-volumes-with-silverstack/)).
  OffShoot MHL re-verification and batch verify of a drive
  ([verification](https://docs.hedge.video/offshoot/features/verification)).
- **Dailix:** a per-day "copies: 1/2" counter with a "make second copy" action, plus a
  "re-verify drive" action that re-hashes against the stored manifest and reports any
  silent damage (bit rot).

### 4.3 Clearing TMP safely
- **What:** OffShoot recommends *Source* verification when you plan to erase sources
  (link above).
- **Dailix:** "Free TMP" is enabled only for days whose archive copies are verified.
  It lists what will be removed and what is kept (e.g. proxies for the sheet).

---

## 5. Top 10 for Dailix (prioritised)

1. **Content-hash duplicate detection and an "already in archive" dimmed state** in the
   ingest grid (OffShoot, Lightroom). This fixes re-imports from phones.
2. **Full read-back verification with one clear end state** ("verified — safe to
   format") and a checksum manifest per day (OffShoot, Silverstack).
3. **Home = trips → day cards**, with clip counts, share rated and storage state
   (Apple Photos Trips, Immich timeline). This view replaces the file manager.
4. **Keyboard culling: P / ? / X with auto-advance** (Photo Mechanic, Lightroom).
5. **I/O range plus a role key**, drawn as coloured bars on the tile filmstrip
   (FCP range keywords, Kyno subclips).
6. **Roles and flags as automatic smart collections** across days (FCP keyword
   collections, Lightroom smart collections).
7. **"Play the day" source tape**, filtered to take and think (Resolve Cut page).
8. **Per-clip location chips `TMP · ARCHIVE n/2 · PROJECT`** with day and trip roll-up
   (Silverstack resources).
9. **Offline volumes stay browsable**, identified by UUID, with auto-relink
   (Lightroom, Canister catalogs).
10. **Story pane with clip references** next to the sheet (no product does this well;
    this is the distinctive feature).

Later: sync survey of 2–4 clips, similarity grouping, map view, cloud target.

---

## 6. Sketches

### 6.1 Home / library
```
┌ Dailix ──────────────────────────────────────────────────────────────────┐
│ [+ Ingest]   Search…                         Filters: ☐ unrated ☐ 1 copy │
├──────────────────────────────────────────────────────────────────────────┤
│ STORAGE  TMP 412 GB free ● │ Archive-A 1.2 TB ● │ NAS ○ offline │ Cloud — │
│          ⚠ 3 days only in TMP                                            │
├──────────────────────────────────────────────────────────────────────────┤
│ ▼ Carpathians 2026  (Oct 03–09 · 7 days · 1 240 clips)        copies 1/2 │
│  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐          │
│  │ img  │ │ img  │ │ img  │ │ img  │ │ img  │ │ img  │ │ img  │          │
│  │Oct 03│ │Oct 04│ │Oct 05│ │Oct 06│ │Oct 07│ │Oct 08│ │Oct 09│          │
│  │212 ▪▪│ │180 ▪▪│ │ 96 ▪ │ │ 140  │ │ ...  │ │ ...  │ │ ...  │          │
│  │✓ A  ✎│ │✓ A  ✎│ │TMP ✎ │ │TMP   │ │      │ │      │ │      │          │
│  └──────┘ └──────┘ └──────┘ └──────┘ └──────┘ └──────┘ └──────┘          │
│   ▪ = share rated   ✎ = story written   ✓A = archived on Archive-A       │
│ ▶ Lviv weekend 2026  (Sep 12–14 · 3 days)                     copies 2/2 │
│ ▶ Unsorted (no trip yet) · 2 days                                        │
└──────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Ingest
```
┌ Ingest ──────────────────────────────────────────────────────────────────┐
│ SOURCES                  │  TO: TMP / Carpathians 2026 / 2026-10-05 / …  │
│ ● DJI Osmo 360 (SD 128G) │  Trip: [Carpathians 2026 ▾]  (auto: Oct 05)   │
│ ● Pixel phone (MTP)      │  Path preview: …/2026-10-05/osmo360/CAM_0012… │
│ ○ Folder…                │                                               │
├──────────────────────────┴───────────────────────────────────────────────┤
│ Oct 04  ☐ 38 clips  ░░ all already in archive (dimmed)                   │
│ Oct 05  ☑ 96 clips  ▓▓▓▓▓▓▓▓▓░░░  new: 91  dup: 5 (skipped)              │
│   [▢][▢][▢][▢][▢][▢][▢][▢][▢][▢] … (hover to skim, space to untick)      │
├──────────────────────────────────────────────────────────────────────────┤
│ JOB  Osmo 360 → TMP     copy ███████░░░ 64%  48 GB/75 · 210 MB/s · 2:10  │
│                         verify ███░░░░░ 31%                              │
│ JOB  Pixel → TMP        ✓ 143 files verified · log                       │
│ ─────────────────────────────────────────────────────────────────────────│
│ RESULT  Pixel: verified in TMP — safe to delete from phone               │
└──────────────────────────────────────────────────────────────────────────┘
```

### 6.3 Contact sheet
```
┌ Carpathians 2026 › Oct 05 ── [Grid] [List] [▶ Play day] ─────────────────┐
│ Filter: [take][think][junk] role:[B][T][C][R] 🚩 note  device:[all ▾]    │
├──────────────────────────────────────────────┬───────────────────────────┤
│ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐   │ STORY — Oct 05            │
│ │ thumb  │ │ thumb  │ │ thumb  │ │ thumb  │   │ Start at the hut, fog.    │
│ │▬B▬ ▬T▬ │ │  ▬R▬   │ │        │ │▬C▬     │   │ @0012 talk — Ivan on      │
│ │TAKE 🚩 │ │THINK   │ │ junk   │ │TAKE    │   │ route. Climb is slow…     │
│ │"fog…"  │ │        │ │        │ │"ridge" │   │ @0019 B-roll ridge        │
│ └────────┘ └────────┘ └────────┘ └────────┘   │                           │
│ ┌────────┐ ┌────────┐ ...                     ├───────────────────────────┤
│ │  ...   │ │  ...   │                         │ CLIP 0012  02:41  osmo360 │
├──────────────────────────────────────────────┤ TMP ✓ · ARCHIVE 0/2       │
│ VIEWER  ▶ 0012 ──[I====B-roll====O]────▮──    │ Segments:                 │
│ keys: P take  ? think  X junk  I/O range      │  00:12–00:40 B  "fog"     │
│       B/T/C/R role  F flag  N note  ⏎ next    │  01:05–01:50 T  "route"   │
└──────────────────────────────────────────────┴───────────────────────────┘
```
