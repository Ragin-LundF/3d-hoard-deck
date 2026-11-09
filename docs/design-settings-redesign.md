# Settings redesign: design notes

Status: proposal by Kreta for Jörg (2026-11-09). Bob implements after approval.
Prototype: https://claude.ai/artifact/QYnzhzWySVSqHZ7h44e2gn (source `design/settings-redesign-prototype.html`, dark + light, toggles for AI on/off, update available, scan problems, managed library).

## Problem

`settings-view.tsx` renders everything on one long page: every library with all its options (access, metadata, rebuild, Orynt3D, suggested collections, folder rules), the file journal, slicers, index, pinned tags, the full tag rule editor, the full AI form and language. With two libraries the page is several screens long and the important state (is anything wrong? what is read-only?) is buried.

## Structure

Settings become pages with their own sidebar. When the route is `settings`, the app sidebar shows the settings navigation instead of the folder tree (like macOS System Settings), so there is no third column.

Sidebar (top to bottom):

- **Back to library** (returns to the previous route) and **Find a setting** (filters to a list of matching settings, see below).
- **Overview**
- LIBRARIES: **Libraries** (count; amber dot when a scan had unreadable files), **File changes** (only when a library has access `managed`)
- ORGANIZE: **Tags & rules** (count of active rules), **AI suggestions** (pill On/Off)
- OPEN: **Slicers** (count)
- APP: **General**, **Index & storage**, **Updates** (accent dot when an update is available)

Blue dot on Tags & rules / AI = unsaved draft on that page.

## Pages

| Page | Content (existing components) |
|---|---|
| Overview | "Needs attention" callouts (unreadable files per library → opens that library; update available → Updates). Then one card per page with a 2–3 line status summary; the whole card is the link. |
| Libraries | Compact list: icon (NAS vs. local if known, else folder), name, root, pills for access mode, metadata storage, model count, scan status/progress, Rescan button, chevron. **Add library folder** as primary button in the page header. |
| Library detail | Breadcrumb `Libraries › Name`, header with Rescan + Full rescan. Scan problems panel (existing `failures` block) above the tabs, with Copy list and Dismiss. Sub-tabs: **Access & storage** (the two radio groups + callout), **Collections & folder rules** (`LibraryRules`, count in tab), **Import & rebuild** (Rebuild, Orynt3D, plus a "Remove library…" danger row if/when that command exists). |
| File changes | `FileJournal`, unchanged. |
| Tags & rules | Sub-tabs **Tag rules** (`TagRuleSettings` as a table: switch, tag, match kind, pattern, model count, options ⚙ and remove; "Unless" and "Only own name" fold into an options row, collapsed rows show them as small pills) with **Try a path** as its own panel below; **Display** (pinned tags switch, link to the tag browser). The "Tag bar" switch in the prototype is optional. |
| AI suggestions | Off: one empty-state panel ("AI suggestions are off … sends nothing anywhere") with provider buttons (LM Studio, Ollama, OpenAI, Anthropic, Other) that turn it on with that provider preselected. On: sections **Connection** (service, address, model, key, Test connection, model chips from the test), **What is sent** (checkboxes, cloud warning, Advanced in `<details>`), **Suggest tags now** (per-library run rows, hidden while unsaved). "Turn off" in the page header. |
| Slicers | `SlicerSettings`, with a drag handle for order (Make default stays as the keyboard path). |
| General | Language select, Theme segmented control Light / Dark (/ System, optional). |
| Index & storage | `IndexSettings` + "Show" in Finder; optional row with sizes (DB, search index, thumbnails) if cheap to compute. Callout: index-only libraries live only here, back it up. |
| Updates | From the update plan § 5: available-version card (version, date, 3 lines What's new, Update now / All changes / Skip this version), installed version + platform, Check now with inline result, Check on start switch, skipped-version row with Show again. |

## Behaviour

- **Routing**: extend the settings route with `tab` and optional `lib` (e.g. `view=settings&tab=library&lib=1&sub=collections`), so the Overview callouts, the update pill and the folder context menu can deep-link (e.g. "Mark as collection… → Library › Collections").
- **Saving**: everything saves at once as today, except Tag rules and AI, which keep their drafts. Their Save/Discard move into a sticky **save bar** at the bottom of the page ("Tag rules: unsaved changes · Discard · Save"). Drafts survive switching pages (lift the draft state out of the page component, e.g. a small store), and the sidebar shows the blue dot.
- **Find a setting**: a static list of `{label, page, sub}` (strings via `t()`), substring match on label and page title, result click navigates and briefly highlights the target panel. No full-text index needed.
- **AI hidden when off** still holds outside settings; in settings the page is always there because it is where AI is turned on.
- Width: content `max-width` ~920px, same panel / `prow2` styles as now. New CSS: settings nav (reuse `.nav`), `.subtabs`, `.cards/.card`, `.lib` list row, `.savebar`, `.sw` switch, `.seg`.
- Both themes use the existing tokens only; no new colours.

## Open questions for Jörg

1. Settings nav replaces the folder sidebar while in settings (proposed), or settings tabs inside the main area with the folder tree kept?
2. Theme "System" option and index size row: add now or later?
