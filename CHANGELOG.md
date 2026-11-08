# Changelog

All notable changes to 3D Hoard Deck are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- **Scan this folder:** a button in the folder header and an entry in the folder's right-click menu read just that
  folder and its subfolders again. Much faster than a full scan in big libraries; the rest of the library stays as
  it is, and the full scan is still there.
- **Screenshot** button in the 3D preview: turn and zoom the model, then save that view as `preview_<name>.png` next
  to the mesh file. In a folder without images it becomes the cover right away; only that folder is updated, so
  there is no wait for a library scan. Existing files are never replaced, and libraries set to "never change files"
  do not allow it.
- **Turn buttons** in the 3D preview: ▲ ◀ ▶ ▼ turn the view in 15° steps, and Front, Side and Top look at the model
  straight on. The mouse works as before.
- Folder rules (one model, collection, accepted suggestions) are saved in the `.3dhoarddeck` at the library root
  when metadata files are on, and **Rebuild from metadata files** restores them on a new computer.
- A folder's own pictures that belong to no model (for example an overview next to model subfolders) are shown
  above its cards and open in a viewer with zoom, without having to mark the folder as one model.
- Such a folder also uses one of its own pictures as its card cover (an image named cover, preview, main or render
  first, else the largest), before falling back to the cover of a model inside.

### Fixed
- Pictures lying next to a single model file in a folder that also holds model subfolders were not shown anywhere;
  they now belong to that file's model (for example `battlestar1.png` next to `Hex3d_CylonHelmet_Uncut.stl`).

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

[Unreleased]: https://github.com/Ragin-LundF/3d-hoard-deck/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Ragin-LundF/3d-hoard-deck/releases/tag/v0.1.0
