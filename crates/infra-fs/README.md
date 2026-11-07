# hoarddeck-infra-fs

File system adapters: the parallel library walker with content sniffing (`FsLibraryScanner`) and the
index location bootstrap (`~/.3dhoarddeck/config.json`). Library files are only ever opened for reading.
Allowed dependencies: hoarddeck-domain, hoarddeck-application, rayon, serde.
