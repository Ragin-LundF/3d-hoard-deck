# Changelog

All notable changes to 3D Hoard Deck are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### New
- **Right-click menus everywhere.** Folders, model cards, pictures and files on a model page, collections, tags and
  duplicates have their own menu (open, open in your slicer, show in folder, copy path, favorite, collections, tag
  actions, …), the same wherever the thing appears. The browser's own menu ("Reload", "Save Image") no longer shows
  up; text fields keep Cut, Copy and Paste. The folder menu is also behind a new "⋯" after the folder path.
- **Rebuild everything from the library folders.** Where metadata files are allowed, the `.3dhoarddeck` files now hold
  everything you decided: names, tags, notes, marks, covers, the date a model was added, "no rendered preview",
  folder marks and folder covers (kept in the folder itself, so they follow a folder renamed outside the app), and at
  the library root the scan rules, tag groups, pinned tags, collections and a copy of the tag rules. Settings ›
  Library › Import & rebuild has **Write metadata files now** (only changed files are written) and **Restore from
  metadata files** ("newer wins" or "files win"). Adding a folder that carries saved data offers to restore it after
  the first scan, so a lost index or a new computer starts with everything in place. Read-only libraries get
  **Export backup file**: one file with the same data, saved anywhere. A metadata file that cannot be written (NAS
  offline) no longer stops the change; it is written later.
- **Extract ZIP, RAR and 7z archives.** A model page with archives, and a folder with loose archives, offer
  **Extract…** for one archive or **Extract all…** (untick the ones to leave packed); a long extraction can run in
  the background while you browse on. Each archive goes into a new folder named after it (with a number if the name is taken; a single
  folder inside the archive is not nested twice). Files appear only once everything is unpacked and every checksum
  matched; a damaged archive, a wrong password, too little space or cancelling leaves nothing behind. Then you check
  the result and choose: **Delete the archives** (into the trash, or for good on a network share after a red
  warning), **Keep archives and folders**, or **Discard the extracted folders** (into the trash too; for good only after a second question). Tags and notes of a deleted archive
  move to its new folder. Needs file changes allowed for the library ("rename and move"). Multi-volume RAR sets and
  password-protected archives work; names Windows and NAS shares cannot store are adjusted and listed, and a few
  pictures of the result are shown before you decide. Only the archive's folder is read again afterwards.

## [1.1.0] - 2026-11-09

This release is about tags without typing and pictures for every card: tag rules and optional AI suggestions,
fixing many tags at once, and covers borrowed from the folder above, read from 3MF files or rendered from the model.

### New
- **A new way to work with tags.** The sidebar keeps your folders in view however many tags you have: one **Tags**
  entry opens the tag browser, and the few tags you pin appear below the folders.
- **Tag bar above the grid.** It shows the most common tags of what you are looking at. Click a tag to see only
  models with it, ⌥-click (Alt-click) to hide models with it, and switch between "All" and "Any" when you pick
  several. **All tags…** finds any tag by typing. The search box shows the filter, so you can save it as a smart
  collection.
- **Tag browser.** Filter, sort by use, name or recent use, list or cloud view, and jump by letter. Select several
  tags to merge, rename, pin, delete or put them into groups such as Scale or Genre. Tags that look alike (Sci-Fi and
  SciFi, Dragon and Dragons) are offered for merging. Only the app's index (and metadata files, if on) change, never
  your model files.
- Search understands `-tag:bust` (without a tag) and `tag:32mm|28mm` (any of these tags).
- **Tags without typing: tag rules.** Settings › Tags › Tag rules turns folder and file names into tags, such as
  `32mm`, `presupported`, `resin`, `bust` or the creator's folder. Add the suggested rules or write your own (any of
  these words, all of them, a regular expression, or the folder below a given folder). Only whole words count
  (`bust` never matches "Robust"), words like "unsupported" can stop a rule, and the usual miniature scales are the
  only sizes. **Count matches** and **Try a path** show what a rule does before you save it. Rule tags are searchable
  and appear in the tag bar, but they are kept apart from your own tags: changing a rule never touches what you typed,
  and nothing is written into your libraries.
- On a model page, rule tags appear under your tags: keep one as a normal tag, or hide it on that model.
- **Tag suggestions from an AI (optional).** Under Settings › AI choose LM Studio, Ollama, OpenAI, Anthropic or another
  OpenAI-compatible server. The AI looks at the model's preview picture and name and suggests a few tags; it never
  changes anything by itself. Ask for one model on its page, for a folder with **Suggest tags**, or for all models
  without tags. Keep or dismiss suggestions per model, or for many models at once in the tag browser. The API key is
  kept in the system keychain. Model files are never sent. As long as AI is off, the app shows no AI feature at all.
- **Update check.** A few seconds after start the app asks GitHub whether a newer version is out. If so, a small
  **Update** pill appears in the title bar: see what's new, then **Update now** (downloads, installs and restarts),
  **Later** or **Skip this version**. Every download is checked against the project's signature before it is installed.
  Settings › Updates shows the installed version, can check right away, and can turn the check off.
- **Settings in pages.** Settings now have their own sidebar (Overview, Libraries, Tags & rules, AI suggestions,
  Slicers, General, Index & storage, Updates) instead of one long page. The overview shows at a glance what needs
  attention, each library has its own page with tabs, **Find a setting** jumps to any option, unsaved tag rule and AI
  changes wait in a bar at the bottom (and survive switching pages), slicers can be reordered by dragging, and the
  theme can be chosen under General.
- **Review AI tags.** A new screen (sidebar › Review AI tags, only while AI is set up) shows the AI's suggestions with
  the model pictures. **By tag:** pick a suggested tag, see every model it was suggested for, switch it off where it
  doesn't fit (⇧-click for a range), then keep it on the rest in one go. **By model:** all suggestions of each model,
  keep the rest or dismiss, also for several models at once. Every step can be undone, also later from **Reviewed**.
- **AI folders.** Choose per library which folders the AI works on (untick subfolders to leave them out), only models
  without own tags, whether new models get suggestions after each scan, and whether models without a picture are
  skipped. The screen shows how many models that is and about how long it takes.
- **Fix many tags at once.** Click **Select** (or ⌘/Ctrl-click, ⇧-click for a range, ⌘A) and use **Add tag** or
  **Remove tag** on all selected models; in a search, **Select all** covers every result, also those not loaded yet.
- **Check a tag.** The ▾ on an active tag in the tag bar opens **Check this tag…**: every model with it, grouped by
  folder, with where it comes from (tag rule or added by you). Mark the wrong ones, or a whole folder, and remove
  the tag from them in one go. If many come from a tag rule, the app points you to the rule. **Remove from all
  here…** does it for everything shown. Every batch can be undone.
- Removing a tag that a tag rule gives hides it on those models, so it doesn't come back with the next scan.
- **Far fewer cards without a picture.** A model without a picture of its own now borrows one from the folder above
  (or the one above that, never the library root), marked **from folder** on the card. 3MF files from Bambu Studio
  and OrcaSlicer show the preview picture stored inside them. Borrowing from the folder above is **on by default**;
  turn it off per library under Settings › Libraries › (library) › Collections & folder rules.
- **Rendered previews.** Models that still have no picture get a preview rendered from their 3D file: grey on light
  grey, with a dashed frame and a **rendered** badge, so it is never mistaken for a photo. Only cards on screen are
  rendered, one at a time; files over 50 MB only when you ask. Hover a card to **Save** it (written as
  `preview_<file>.png` into the model folder) or, in a read-only library, **Keep** it in the app; ✕ skips the model
  for good. **Save all** / **Keep all** above the grid does it for all of them. Keep and skip can be undone.
  Settings › General chooses whether previews render for models on screen, only when you ask, or never.
- **Scroll through a model's pictures with the mouse.** The picture strip on the model page has arrows on the left
  and right when there are more pictures than fit.

### Improved
- **Marking folders no longer makes you wait.** "Treat as one model", "Treat as collection", accepting a suggestion
  or removing a rule now take effect at once; the models are regrouped in the background. Marking several folders
  in a row regroups only once, and a running scan is simply waited for.
- **Regrouping is about four times faster.** Only the models that actually change are updated, instead of all of
  them. In a library with about 45,000 models this takes about half a second instead of more than two.
- **Scans write far less.** Folders that did not change cost no work in the app's index any more, which saves
  several seconds per scan in big libraries, and the app now asks the NAS for more folders at the same time.
- **Pictures load quickly during a scan.** Pictures and 3D files now go first: while you wait for one, a running scan
  steps back for a moment, so browsing stays smooth while the library is being read.
- The first start after the update takes a few seconds longer once while the app prepares its index.

### Fixed
- Clicking a folder in the path at the top sometimes opened "Everything inside" instead of the folder, depending on
  where you came from. It now always opens the folder, like the sidebar.
- "Treat as one model" and the other folder choices did nothing while a scan was running. They now take effect
  during the scan.

## [1.0.0] - 2026-11-08

3D Hoard Deck is ready for everyday use. This release is about finding your models faster, giving every folder a
nice picture, and making big libraries on a NAS feel quick.

### New
- **Make your own cover pictures.** Turn a model in the 3D preview until it looks right and press **Screenshot**.
  The picture is saved next to the file and shows up as the cover straight away, with no waiting for a scan.
- **Folder pictures like in a movie library.** Folders without a picture now borrow one from their subfolders. You
  can also choose one yourself: show an image on a model page and press **Use for folder…**. If the library allows
  it, the picture is copied into that folder, so it stays even on a new computer. Right-click a folder and choose
  **Automatic cover** to go back.
- **Scan just one folder.** Added or changed a few files? Open the folder and press **Scan this folder**, or use the
  folder's right-click menu. Only that folder is read again, which takes seconds instead of minutes in a big
  library.
- **Turn the 3D view precisely.** Arrow buttons turn the model step by step, and Front, Side and Top show it
  straight on. Mouse and trackpad work as before.
- **Pictures in a folder are visible.** Pictures that lie in a folder next to model subfolders, such as an overview
  of a release, now appear above the folder's cards and open in a large viewer with zoom.

### Improved
- When metadata files are switched on, your folder decisions (one model, collection, accepted suggestions) are
  saved too, and **Rebuild from metadata files** brings them back.
- A folder that was deleted outside the app disappears after **Scan this folder**, instead of being reported as
  unreadable.
- The large picture viewer can be used with the keyboard alone.

### Fixed
- Pictures next to a single model file were not shown when the same folder also had model subfolders. They now
  appear on that model's page.

## [0.1.0] - 2026-11-07

The first release: your 3D printing library, searchable and good-looking, right where your files already are.
3D Hoard Deck indexes folders of STL, 3MF and OBJ files in place: nothing is moved, copied or uploaded, and the app
never writes into a library unless you allow it.

### Added

#### Browsing and search
- Browse folders as they are, with breadcrumbs, a folder tree and an "Everything inside" view.
- One card per printable thing: variant folders such as `Supported`, `Unsupported`, `images` or `75mm` are merged
  into their model.
- Right-click a folder to treat it as one model or as a collection when the automatic detection gets it wrong.
- Release folders like `2024-03 - Spring Release` show as "Spring Release" with a date badge.
- Sort by name, recently added or release date, and change the card size.
- Fast search that tolerates typos and partial words, with filters such as `tag:resin`, `type:3mf`, `is:favorite`,
  `is:printed` and `in:"Miniatures/Forge Masters"`, optionally limited to the current folder.

#### Covers and images
- Covers from images in the model folder, or next to a single file with the same name, resized once and cached.
- Pick any image of a model as its cover.
- An image view that fits images to the window, zooms up to 8× (buttons, mouse wheel or trackpad pinch) and lets
  you drag a zoomed image around.

#### 3D preview
- Rotate, zoom and pan STL, 3MF and OBJ files, solid or wireframe, with a 256 × 256 mm build plate, size in
  millimetres and the triangle count.
- 3MF projects from Bambu Studio and OrcaSlicer show every build plate side by side, or one plate at a time.
- A simplified preview for STL files over 128 MB; the slicer still receives the original.
- STL and OBJ files inside ZIP archives can be listed and previewed without unpacking.
- Disguised files (`.stl.xxx`, `.stl.txt`) are recognized by their content.

#### Slicers
- Open a model in Bambu Studio, OrcaSlicer, PrusaSlicer, Lychee or any other slicer with one click; 3MF projects
  are preferred so plates and settings come along.
- Find installed slicers automatically, add any other program, and choose the file types each slicer opens.

#### Organizing
- Rename models inside the app; the files on disk stay as they are.
- Tags, notes, favorites and "printed" marks, with Favorites, Printed and the most used tags in the sidebar.
- Hand-made collections in your own order, and smart collections that save a search.
- Suggested collections for common pack layouts, such as Loot Studios releases, STLFlix and monthly creator
  releases.
- A duplicate finder that compares file contents across all libraries, with progress and cancel.
- Import names, notes and tags from Orynt3D (read-only, safe to run again).

#### Safety of your files
- Every library starts as "never change files in this library".
- Optional `.3dhoarddeck` metadata files beside each model, and a rebuild of the index from those files.
- Optional renaming and moving on disk, with a change log and undo. Existing files are never overwritten, and an
  interrupted change is settled at the next start.
- Libraries on a local disk update by themselves a few seconds after files change. Network shares are checked
  every 30 minutes and after the app starts, unless they were scanned within the last hour.
- Folders and files that cannot be read are listed with the reason in Settings, can be copied, and keep their
  previous index data.
- The library stays browsable while a network share is offline.

#### App
- English and German; the language follows the system and can be changed in Settings.
- Light and dark theme.
- All app data in one folder (`~/.3dhoarddeck` by default), movable under Settings › Index.
- Installers for macOS (Apple Silicon and Intel), Windows and Linux, built by GitHub Actions for each release.

### Known limitations
- The builds are not signed with a developer certificate. On macOS, allow the app once under System Settings ›
  Privacy & Security › Open Anyway; on Windows choose More info › Run anyway.
- 3MF files inside ZIP archives are listed but not previewed, and RAR and 7z archives are not opened.
- Very large OBJ and 3MF files (over 128 MB) are not previewed.
- Some messages from deep inside the app, such as rare file-system errors, are still shown in English.
- There are no automatic updates yet.

[Unreleased]: https://github.com/Ragin-LundF/3d-hoard-deck/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/Ragin-LundF/3d-hoard-deck/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/Ragin-LundF/3d-hoard-deck/compare/v0.1.0...v1.0.0
[0.1.0]: https://github.com/Ragin-LundF/3d-hoard-deck/releases/tag/v0.1.0
