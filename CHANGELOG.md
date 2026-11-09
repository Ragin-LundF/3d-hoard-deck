# Changelog

All notable changes to 3D Hoard Deck are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Faster
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

[1.0.0]: https://github.com/Ragin-LundF/3d-hoard-deck/compare/v0.1.0...v1.0.0
[0.1.0]: https://github.com/Ragin-LundF/3d-hoard-deck/releases/tag/v0.1.0
