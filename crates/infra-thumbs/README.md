# hoarddeck-infra-thumbs

Cover thumbnails: library images are decoded once, scaled to 512 px and stored as JPEG in
`<index folder>/thumbnails/`. Keys are derived from the file's path, size and modification time, so a changed
image gets a new thumbnail and deleting the folder is always safe. Reads the library only.
Allowed dependencies: thiserror, image.
