# hoarddeck-cli

Phase 0 spike tool: runs the use cases from the command line (wiring in `hoarddeck-app`) and prints timings.
Only reads libraries; writes go to the index folder (default `~/.3dhoarddeck`, override with `--index`).

    cargo run --release -p hoarddeck-cli -- --index /tmp/hd add "3D Print" "/Volumes/volume/3D Print"
    cargo run --release -p hoarddeck-cli -- --index /tmp/hd scan 1
    cargo run --release -p hoarddeck-cli -- --index /tmp/hd search 1 wolverin type:stl
    cargo run --release -p hoarddeck-cli -- --index /tmp/hd browse 1 Miniatures --all
