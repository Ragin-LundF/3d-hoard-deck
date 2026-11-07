# hoarddeck-infra-platform

Operating-system specifics: starting slicers (a direct process spawn, never a shell string; `open -a` for macOS
app bundles) and finding installed slicers at their default install paths on macOS, Windows and Linux.
Allowed dependencies: hoarddeck-domain, hoarddeck-application.
