# hoarddeck-app

Composition root: builds every adapter once and injects them into the use cases (`App`).
Used by the desktop shell (`src-tauri`) and the spike CLI (`crates/cli`). Contains no logic beyond wiring.
