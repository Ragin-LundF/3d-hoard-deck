# Plan: auto-tagging

Status: proposal for Jörg, to be implemented by Bob. Covers tag *suggestions* only; the tags UI redesign is a separate piece of work.

## Goal

Most of the 46,513 models have no tags (11 have meta today). Tags should appear without typing them by hand:

1. **Without AI**, from what the library already tells us: folder names, file names, creator layouts.
2. **With AI (optional)**, from the preview image plus the model name. STL files are never uploaded.

When no AI is configured, every AI control is hidden. The non-AI part works on its own.

## What the library already tells us

Real paths from the index (`~/.3dhoarddeck/index.sqlite`):

```
Miniatures/Loot Studios/Fantasy/2023-05 - Tales of Ryuboken/resin/32mm/All_Enemies_32mm/.../OniMage_32mm_UnSupported
Miniatures/Loot Studios/SciFi/2024-11 - Cryo Hazard/resin/75mm/.../PitblazerUltraWarden_75mm_ReadytoSlice
Miniatures/MyMiniFactory/Animal Den Miniatures/2026-08 - Animal Collector/Jaguar swimming
Miniatures/Archvillain Games/2022-08 - Agama Shattered Valley/...
```

That is creator, genre, release, scale, print state, resin and hero/enemy, all in the path. Release dates and collections are already parsed; tags are not.

## Phase 1: tag rules (no AI)

The model is the matching in **Paperless-ngx** (the DMS we assume Jörg meant, see open questions): each tag has a match pattern and an algorithm, and matching documents get the tag.

**Rule** = tag name + algorithm + pattern + where to look.

- Algorithms: `any word`, `all words`, `exact`, `regex`, `fuzzy` (same names as Paperless). Skip Paperless "Auto" (machine learning from existing tags): with 11 tagged models there is nothing to learn from.
- Where: the model path (all folder segments plus the model name), case-insensitive, `_`, `-` and CamelCase split into words before matching.
- A regex rule may use its capture as the tag: `(\d+)\s?mm` gives `32mm`, `75mm`.

**Built-in presets** (data, like `scan/presets.rs`; proposed once, the user switches them on):

| Tag | Rule |
|---|---|
| `{1}mm` (scale) | regex `(\d{2,3})\s?mm` |
| `presupported` | any word: presupported, supported, readytoslice, "ready to slice" |
| `unsupported` | any word: unsupported |
| `resin` / `fdm` | any word |
| `bust` | any word: bust |
| `fantasy` / `sci-fi` | any word: fantasy / scifi, sci-fi |
| `hollow` | any word: hollow |
| creator | the folder under `Miniatures/` (or under `MyMiniFactory/`) becomes the tag, e.g. `Loot Studios`, `Animal Den Miniatures` |

Note: "supported" must be checked after "unsupported", since the word split makes `UnSupported` contain both. The simplest way is to give rules an order and let `unsupported` win.

**Where rule tags live.** Store them apart from the user's own tags: a new column `models.auto_tags` (JSON array), filled during the scan and recomputed whenever a rule changes. Reasons: changing a rule never touches what the user typed, and nothing is written into a read-only library. Search indexes both into the same Tantivy `tag` field, so `tag:32mm` finds either. The UI shows auto tags as a lighter chip; "keep" copies one into `model_meta.tags`, "hide" adds it to a per-model hidden list. Needs migration 3 in `infra-sqlite/src/schema.rs` (never edit migration 1/2).

Rules are stored in `app_settings` under key `tag_rules`, like `slicers` today. A rule change triggers recomputation from the index only (paths are already there, no disk access needed), so it is fast and safe on the read-only NAS.

**Settings UI**: "Tag rules" section with the rule list, a test field ("type a path, see the tags"), and a match count per rule ("matches 4,210 models") before saving.

## Phase 2: AI suggestions (optional)

### Settings ("AI" section)

Modelled on paperless-gpt / paperless-ai, which put an OpenAI-compatible LLM behind Paperless:

- Provider preset: **Off** (default), **LM Studio** (`http://localhost:1234/v1`), **Ollama** (`http://localhost:11434/v1`), **OpenAI** (`https://api.openai.com/v1`), **Custom** (any OpenAI-compatible base URL).
- Model: dropdown filled from `GET {base}/models`; free text as fallback.
- API key: optional, stored in the macOS Keychain (`keyring` crate), never in SQLite, never sent to the webview.
- Send: preview image (on by default) and name + folder path (on by default).
- Max new tags per model (default 3); suggestions prefer the existing tag vocabulary.
- Prompt: built in, with an "advanced" override using placeholders `{name}`, `{path}`, `{tags}`.
- **Test connection** button: sends one bundled sample image and shows the answer and the time.
- A remote provider shows one line: "Preview images and model names are sent to this service."

The UI gets `aiEnabled: bool` from one settings DTO; when false, no AI button, menu item or column renders anywhere.

### Request

One OpenAI chat-completions call per model:

- Image: the existing 512 px JPEG from `ThumbnailCache::get_or_create` as a `data:image/jpeg;base64,…` URL. No new image work.
- Text: name, relative folder path, up to ~200 most used existing tags, max new tags.
- `response_format: json_schema` with `{tags: string[], description: string}`.
- The prompt tells the model **not** to guess scale, support state or print technology: those come from rules (see test results, it mis-tagged a display stand as "supported").

Parsing must be tolerant (found in the LM Studio test): read `message.content`; if empty, read `message.reasoning_content`; strip ```` ```json ```` fences. Then normalise each tag through `TagName::new`, lowercase, dedupe, and map case-insensitively onto existing tags.

### What happens with the answer

AI results are **suggestions**, never applied silently. They are stored per model (`models.ai_suggestions` JSON plus the model/provider used and a timestamp, same migration) and shown next to the auto tags with "keep / dismiss". A review screen lists suggested tags across many models so the user can accept one tag for all of them at once.

### Running it

- Scopes: one model (button on the model page), a folder or collection (context menu), "untagged models in this library".
- A background job like the scan: cancellable, progress in the status bar, one request at a time by default (LM Studio allows 4 in parallel; make it a setting later only if needed), resumes by skipping models that already have suggestions from the same model.
- Never "tag everything" by default: at ~2.7 s per image the 22,606 models with a cover take about 17 hours locally.
- Models without a cover (24k): text-only request from name and path (much cheaper), or skip. A later option is rendering a snapshot with the existing 3D viewer and caching it as the cover thumbnail.

### Code placement (fits the current layers)

- `application`: port `TagSuggester` (`suggest(image: Option<&Path>, name, path, vocabulary) -> Result<Vec<TagName>>`), use cases `SuggestTags` (job) and `ApplyTagRules`.
- New crate `infra-ai`: the OpenAI-compatible client. Needs an HTTP client; **`ureq`** (blocking, small, rustls) fits the existing blocking use-case style. Calls happen in Rust so the key never reaches the webview.
- `src-tauri`: commands `ai_settings`, `save_ai_settings`, `test_ai_connection`, `list_ai_models`, `suggest_tags`, `cancel_suggest_tags`, `tag_rules`, `save_tag_rules`, `test_tag_rule`.
- Every new UI text needs a German entry (`ui/src/shared/i18n-de.ts`), or vitest fails.

## LM Studio test (2026-11-09, on Jörg's Mac)

Model `qwen3.8-3.6-27b-blend-mlx` (vision, loaded). LM Studio's server was not running; I started it with `lms server start` (port 1234). Four 512 px thumbnails from `~/.3dhoarddeck/thumbnails`, sent without a name:

| Image (what it is) | Tags returned | Verdict |
|---|---|---|
| Walking lion, Animal Den | miniature, animal, lion, walking | correct |
| Armored rabbit with gatling gun | miniature, sci-fi, animal, robot, mech, gun | correct |
| Female sci-fi bust with pistol | bust, sci-fi, miniature, female, gun | correct |
| Tardigrade on display stand | miniature, animal, supported | correct, it even named the tardigrade in the description; "supported" is wrong |

Findings:

- **Speed:** 2.6–2.8 s per image with `json_schema`. Without it the model "thinks" first: 42–45 s and ~1,000 reasoning tokens. Asking to disable thinking (`chat_template_kwargs`, `reasoning_effort`) had no effect, so `json_schema` is required, not optional.
- With `json_schema`, LM Studio puts the JSON into `reasoning_content` and leaves `content` empty. Hence the tolerant parsing above.
- Name helps a little ("walking" came from the name in one run), but the image alone is already good.

Test script: the scratchpad `tagtest.py` (Python, stdlib only); easy to turn into the Rust client's integration test against a running LM Studio.

## Order of work for Bob

1. Migration 3 (`auto_tags`, `hidden_auto_tags`, `ai_suggestions`) + search indexing of auto tags.
2. Tag rules: model, matcher, presets, settings commands, recompute on change, settings UI with test field. Ship this alone first; it is useful without AI.
3. AI settings + Keychain + test connection; `aiEnabled` hides all AI UI.
4. `infra-ai` client + `SuggestTags` single-model + model-page button.
5. Batch job with scopes, progress, cancel; review screen.

Definition of done per step: unit tests for the matcher (word split, regex capture, unsupported vs supported) and the response parser (content, reasoning_content, fenced JSON, invalid tags); the integration test against LM Studio is ignored by default.

## Open questions for Jörg

1. Which DMS did you mean? This plan assumes **Paperless-ngx** (matching algorithms) with paperless-gpt/paperless-ai (LLM settings).
2. Auto tags apart from your own tags (proposed), or written straight into the normal tags?
3. Creator as a tag, or is the existing folder/collection enough?
