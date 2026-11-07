# hoarddeck-infra-mesh

Mesh file readers that are too heavy or too format-specific for the webview. Today: 3MF build plates
(core spec, the production extension's per-object files, and Bambu/Orca plate assignment from
`Metadata/model_settings.config`). Reads only; never writes. Allowed dependencies: thiserror, zip, quick-xml.
