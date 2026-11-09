//! Reading what a model answered. Servers and models differ: LM Studio puts schema answers into
//! `reasoning_content`, models wrap JSON in code fences or add a sentence, and some ignore the schema and
//! write a plain list.

use serde_json::Value;

const MAX_WORDS: usize = 3;

/// The answer text of a chat completion: `content`, or `reasoning_content` when that is empty.
pub(crate) fn openai_text(body: &Value) -> String {
    let message = &body["choices"][0]["message"];
    let text = |field: &str| message[field].as_str().unwrap_or_default().trim().to_owned();
    let content = text("content");
    if content.is_empty() { text("reasoning_content") } else { content }
}

/// The text blocks of an Anthropic message.
pub(crate) fn anthropic_text(body: &Value) -> String {
    body["content"]
        .as_array()
        .map(|blocks| {
            blocks.iter().filter(|block| block["type"] == "text").filter_map(|block| block["text"].as_str()).collect()
        })
        .unwrap_or_default()
}

/// The tags in an answer: the `tags` of the outermost JSON object, else a comma- or line-separated list.
pub(crate) fn tags_of(text: &str) -> Vec<String> {
    if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}'))
        && start < end
        && let Ok(json) = serde_json::from_str::<Value>(&text[start..=end])
        && let Some(tags) = json["tags"].as_array()
    {
        return tags.iter().filter_map(Value::as_str).map(str::to_owned).collect();
    }
    text.lines()
        .filter(|line| !line.trim_start().starts_with("```"))
        .map(|line| line.split_once(':').map_or(line, |(_, rest)| rest))
        .flat_map(|line| line.split([',', ';']))
        .map(|part| {
            part.trim().trim_start_matches(['-', '*', '•']).trim().trim_matches(['"', '\'', '`', '.']).to_owned()
        })
        .filter(|part| !part.is_empty() && part.split_whitespace().count() <= MAX_WORDS)
        .collect()
}

/// Model names a server lists, without those that cannot chat (embedding and reranking models).
pub(crate) fn chat_models(body: &Value) -> Vec<String> {
    const NOT_FOR_CHAT: [&str; 5] = ["embed", "rerank", "e5-", "bge-", "gte-"];
    body["data"]
        .as_array()
        .map(|models| {
            models
                .iter()
                .filter_map(|model| model["id"].as_str())
                .filter(|id| !NOT_FOR_CHAT.iter().any(|part| id.to_lowercase().contains(part)))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn content_falls_back_to_reasoning_content() {
        let lm_studio =
            json!({"choices": [{"message": {"content": "", "reasoning_content": "{\"tags\": [\"lion\"]}"}}]});
        assert_eq!(tags_of(&openai_text(&lm_studio)), ["lion"]);
        let normal = json!({"choices": [{"message": {"content": "{\"tags\": [\"bust\"], \"description\": \"x\"}"}}]});
        assert_eq!(tags_of(&openai_text(&normal)), ["bust"]);
    }

    #[test]
    fn fenced_json_and_plain_lists_are_understood() {
        assert_eq!(tags_of("Sure!\n```json\n{\"tags\": [\"dragon\", \"fantasy\"]}\n```"), ["dragon", "fantasy"]);
        assert_eq!(tags_of("Tags: dragon, \"winged\", - fantasy."), ["dragon", "winged", "fantasy"]);
        assert!(tags_of("I think this is a very large dragon with wings").is_empty(), "a sentence is no tag");
    }

    #[test]
    fn anthropic_text_blocks_are_joined_and_embedding_models_skipped() {
        let message = json!({"content": [{"type": "text", "text": "{\"tags\": [\"vase\"]}"}]});
        assert_eq!(tags_of(&anthropic_text(&message)), ["vase"]);
        let models = json!({"data": [{"id": "text-embedding-nomic"}, {"id": "qwen3-vl"}]});
        assert_eq!(chat_models(&models), ["qwen3-vl"]);
    }
}
