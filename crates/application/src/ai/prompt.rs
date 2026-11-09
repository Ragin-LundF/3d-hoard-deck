use hoarddeck_domain::TagName;

use super::AiSettings;
use crate::ports::SuggestRequest;

/// Up to this many existing tags go into the request, most used first.
pub(crate) const VOCABULARY: usize = 200;
const MAX_WORDS: usize = 3;

pub(crate) const DEFAULT_PROMPT: &str = "You tag 3D printable models in a personal library, so the owner can find \
them again by tag. Look at the preview picture and the name.

Choose up to {max} tags:
- what the model is, for example dragon, knight, bust, vase, terrain or tool;
- its subject or genre, for example fantasy, sci-fi, animal, vehicle or horror;
- a striking feature, for example mounted, winged or articulated.

Prefer the existing tags and write them exactly as listed. Add a new tag only if no existing tag fits. A tag is one \
or two words in lower case. Do not tag the scale or size, the support state, the print technology or the file type \
(no 32mm, supported, presupported, resin, fdm, stl): these come from elsewhere. No names of creators, shops or \
releases.

Answer only with JSON: {\"tags\": [\"...\"], \"description\": \"one short sentence\"}.

Existing tags: {tags}";

/// Words the AI must not tag because tag rules or the files already tell them, and it guesses them wrongly
/// (it called a display stand "supported" in the test).
const LEFT_TO_RULES: [&str; 9] =
    ["supported", "presupported", "pre-supported", "unsupported", "resin", "fdm", "stl", "3mf", "obj"];

pub(crate) fn request(
    settings: &AiSettings,
    name: &str,
    path: &str,
    vocabulary: &[TagName],
    image_jpeg: Option<Vec<u8>>,
) -> SuggestRequest {
    let tags = if vocabulary.is_empty() {
        "none yet".to_owned()
    } else {
        vocabulary.iter().take(VOCABULARY).map(TagName::as_str).collect::<Vec<_>>().join(", ")
    };
    let template = if settings.prompt.trim().is_empty() { DEFAULT_PROMPT } else { settings.prompt.as_str() };
    let path = if settings.send_path { path } else { "" };
    let fill = |text: &str| {
        text.replace("{max}", &settings.max_tags.to_string())
            .replace("{tags}", &tags)
            .replace("{name}", name)
            .replace("{path}", path)
    };
    let mut text = format!("Name: {name}");
    if !path.is_empty() {
        text.push_str("\nFolder: ");
        text.push_str(path);
    }
    SuggestRequest { system: fill(template), text, image_jpeg: image_jpeg.filter(|_| settings.send_image) }
}

/// Turns what the AI wrote into tags: tidied, written like an existing tag when one matches, without tags the
/// model already has, without sizes and support words, each once, at most `max`.
pub(crate) fn clean(answer: &[String], vocabulary: &[TagName], existing: &[TagName], max: usize) -> Vec<TagName> {
    let mut tags: Vec<TagName> = Vec::new();
    for raw in answer {
        let raw = raw.trim().trim_matches(|c: char| c == '#' || c == '"' || c == '\'' || c == '.');
        let Ok(tag) = TagName::new(raw) else { continue };
        let key = tag.key();
        if tag.as_str().split(' ').count() > MAX_WORDS || is_size(&key) || LEFT_TO_RULES.contains(&key.as_str()) {
            continue;
        }
        let tag = vocabulary
            .iter()
            .find(|known| known.key() == key)
            .cloned()
            .unwrap_or_else(|| TagName::new(&key).unwrap_or(tag));
        if existing.iter().chain(&tags).any(|known| known.key() == tag.key()) {
            continue;
        }
        tags.push(tag);
        if tags.len() >= max {
            break;
        }
    }
    tags
}

/// `32mm`, `75 mm`, `1:10`, `1/24`.
fn is_size(key: &str) -> bool {
    let compact: String = key.chars().filter(|c| !c.is_whitespace()).collect();
    let digits = compact.trim_end_matches("mm").trim_end_matches("cm");
    (compact.len() != digits.len() && !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
        || compact
            .split([':', '/'])
            .filter(|part| !part.is_empty())
            .all(|part| part.chars().all(|c| c.is_ascii_digit()))
            && compact.contains([':', '/'])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(tags: &[&str]) -> Vec<TagName> {
        tags.iter().map(|tag| TagName::new(tag).expect("valid")).collect()
    }

    #[test]
    fn answers_are_tidied_and_mapped_onto_existing_tags() {
        let answer: Vec<String> =
            ["Dragon", "#Fantasy", "supported", "32mm", "1:10", "Sci-Fi", "a very long tag of words", "dragon", "lion"]
                .iter()
                .map(|tag| (*tag).to_owned())
                .collect();
        let cleaned = clean(&answer, &names(&["Sci-Fi", "Dragon"]), &names(&["lion"]), 10);
        assert_eq!(cleaned.iter().map(TagName::as_str).collect::<Vec<_>>(), ["Dragon", "fantasy", "Sci-Fi"]);
        assert_eq!(clean(&answer, &[], &[], 1).len(), 1, "at most max");
    }

    #[test]
    fn the_request_follows_the_settings() {
        let settings = AiSettings { send_path: false, send_image: false, max_tags: 4, ..AiSettings::default() };
        let request = request(&settings, "Red Dragon", "Minis/Dragons", &names(&["fantasy"]), Some(vec![1, 2]));
        assert!(request.system.contains("up to 4 tags") && request.system.contains("Existing tags: fantasy"));
        assert_eq!(request.text, "Name: Red Dragon");
        assert!(request.image_jpeg.is_none());
    }
}
