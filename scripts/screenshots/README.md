# README screenshots

`./run.sh` rebuilds every picture in `docs/screenshots/` from scratch:

1. `make-demo-library.py` writes a small demo library with real meshes (vases, gears, knots, chess pieces and two
   Bambu-style 3MF projects) into `target/screenshots/library`.
2. The CLI (`hoarddeck add` / `scan`) builds a real index for it in `target/screenshots/index`.
3. `screenshots.mjs covers` opens every model in the app's 3D viewer and saves the view as its `cover.png`; a second
   scan picks the covers up.
4. `seed-demo-metadata.py` adds tags, favorites, printed marks and notes.
5. `screenshots.mjs shots` takes the screenshots (dark theme, 1440 × 900 at 2×) and `light-dark.png`, the library
   in both themes split along a diagonal.

The UI runs from the Vite dev server on port 5179 (so it does not clash with `npm run dev` on 5173); the Tauri backend
is replaced by small handlers that read the demo index. Nothing outside `target/screenshots` and `docs/screenshots`
is written.

Needs cargo, python3, Node 22+ and a Chromium for Playwright: either `npx playwright install chromium` once, or
`CHROME=/path/to/chrome ./run.sh`.

To add a screenshot, add a `shot(...)` line in `screenshots.mjs`; if the screen calls a command the handlers do not
know yet, add a handler that answers from the index.
