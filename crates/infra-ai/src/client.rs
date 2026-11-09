use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hoarddeck_application::ai::{AiProvider, AiSettings};
use hoarddeck_application::ports::{AiAnswer, SuggestRequest, TagSuggester};
use serde_json::{Value, json};

use crate::answer::{anthropic_text, chat_models, openai_text, tags_of};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 1_024;
/// Listing models must be quick even when answers may take long.
const LIST_TIMEOUT: Duration = Duration::from_secs(15);

/// Talks to an `OpenAI`-compatible server (LM Studio, Ollama, `OpenAI`, others) or to Anthropic. Blocking: the use cases
/// run on background threads.
pub struct HttpTagSuggester;

impl TagSuggester for HttpTagSuggester {
    fn models(&self, settings: &AiSettings, key: Option<&str>) -> Result<Vec<String>, String> {
        let body = call(settings, key, "models", None, LIST_TIMEOUT)?;
        Ok(chat_models(&body))
    }

    fn suggest(&self, settings: &AiSettings, key: Option<&str>, request: &SuggestRequest) -> Result<AiAnswer, String> {
        let model = self.model(settings, key)?;
        let timeout = Duration::from_secs(u64::from(settings.timeout_seconds));
        let text = if settings.provider == AiProvider::Anthropic {
            // Like the OpenAI path: a refused answer format is asked again without it.
            match call(settings, key, "messages", Some(&anthropic_body(&model, request, true)), timeout) {
                Err(reason) if reason.starts_with("400") => anthropic_text(&call(
                    settings,
                    key,
                    "messages",
                    Some(&anthropic_body(&model, request, false)),
                    timeout,
                )?),
                answer => anthropic_text(&answer?),
            }
        } else {
            // A schema makes LM Studio answer in seconds instead of "thinking" for a minute; servers that do not know
            // `response_format` refuse it, so they are asked again without.
            match call(settings, key, "chat/completions", Some(&openai_body(&model, request, true)), timeout) {
                Err(reason) if reason.starts_with("400") => openai_text(&call(
                    settings,
                    key,
                    "chat/completions",
                    Some(&openai_body(&model, request, false)),
                    timeout,
                )?),
                answer => openai_text(&answer?),
            }
        };
        let tags = tags_of(&text);
        if tags.is_empty() {
            return Err(format!("no tags in the answer of {model}"));
        }
        Ok(AiAnswer { tags, model })
    }
}

impl HttpTagSuggester {
    /// The model from the settings, else the first chat model the server lists, else the provider's default.
    fn model(&self, settings: &AiSettings, key: Option<&str>) -> Result<String, String> {
        if !settings.model.trim().is_empty() {
            return Ok(settings.model.trim().to_owned());
        }
        if let Some(model) = settings.provider.default_model() {
            return Ok(model.to_owned());
        }
        self.models(settings, key)?
            .into_iter()
            .next()
            .ok_or_else(|| "the server lists no model; load one or name it in Settings".to_owned())
    }
}

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {"tags": {"type": "array", "items": {"type": "string"}}, "description": {"type": "string"}},
        "required": ["tags", "description"],
        "additionalProperties": false
    })
}

fn openai_body(model: &str, request: &SuggestRequest, with_schema: bool) -> Value {
    let mut content = vec![json!({"type": "text", "text": request.text})];
    if let Some(jpeg) = &request.image_jpeg {
        let url = format!("data:image/jpeg;base64,{}", STANDARD.encode(jpeg));
        content.push(json!({"type": "image_url", "image_url": {"url": url}}));
    }
    let mut body = json!({
        "model": model,
        "messages": [{"role": "system", "content": request.system}, {"role": "user", "content": content}],
        "max_tokens": MAX_TOKENS,
    });
    if with_schema {
        body["response_format"] =
            json!({"type": "json_schema", "json_schema": {"name": "tags", "strict": true, "schema": schema()}});
    }
    body
}

fn anthropic_body(model: &str, request: &SuggestRequest, with_schema: bool) -> Value {
    let mut content = Vec::new();
    if let Some(jpeg) = &request.image_jpeg {
        content.push(json!({"type": "image", "source": {"type": "base64", "media_type": "image/jpeg", "data": STANDARD.encode(jpeg)}}));
    }
    content.push(json!({"type": "text", "text": request.text}));
    let mut body = json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "system": request.system,
        "messages": [{"role": "user", "content": content}],
    });
    if with_schema {
        body["output_config"] = json!({"format": {"type": "json_schema", "schema": schema()}});
    }
    body
}

/// GET (no body) or POST to `{url}/{path}`. Errors start with the HTTP status when there is one.
fn call(
    settings: &AiSettings,
    key: Option<&str>,
    path: &str,
    body: Option<&Value>,
    timeout: Duration,
) -> Result<Value, String> {
    let url = format!("{}/{path}", settings.url());
    let agent: ureq::Agent =
        ureq::Agent::config_builder().timeout_global(Some(timeout)).http_status_as_error(false).build().into();
    let anthropic = settings.provider == AiProvider::Anthropic;
    let result = if let Some(body) = body {
        headers(agent.post(&url).header("content-type", "application/json"), key, anthropic).send(body.to_string())
    } else {
        headers(agent.get(&url), key, anthropic).call()
    };
    let mut response = result.map_err(|error| format!("{url} could not be reached: {error}"))?;
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string().map_err(|error| format!("reading the answer failed: {error}"))?;
    if !(200..300).contains(&status) {
        let reason = match status {
            401 | 403 => "the server refused the API key".to_owned(),
            404 => format!("{url} does not exist; check the address and the model"),
            _ => text.chars().take(300).collect(),
        };
        return Err(format!("{status}: {reason}"));
    }
    serde_json::from_str(&text).map_err(|_| format!("the answer of {url} is not JSON"))
}

/// The key (as a bearer token, or Anthropic's `x-api-key`) and Anthropic's API version.
fn headers<B>(mut request: ureq::RequestBuilder<B>, key: Option<&str>, anthropic: bool) -> ureq::RequestBuilder<B> {
    if let Some(key) = key {
        request = if anthropic {
            request.header("x-api-key", key)
        } else {
            request.header("authorization", &format!("Bearer {key}"))
        };
    }
    if anthropic { request.header("anthropic-version", ANTHROPIC_VERSION) } else { request }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run against a local LM Studio with a vision model loaded: `cargo test -p hoarddeck-infra-ai -- --ignored`.
    #[test]
    #[ignore = "needs LM Studio on localhost:1234"]
    fn lm_studio_answers_with_tags() {
        let settings = AiSettings { provider: AiProvider::LmStudio, ..AiSettings::default() };
        let request = SuggestRequest {
            system: "Tag 3D printable models. Answer with JSON {\"tags\": [...], \"description\": \"...\"}.".to_owned(),
            text: "Name: Red dragon bust".to_owned(),
            image_jpeg: None,
        };
        let answer = HttpTagSuggester.suggest(&settings, None, &request).expect("answer");
        assert!(!answer.tags.is_empty(), "{answer:?}");
    }

    #[test]
    fn requests_carry_the_picture_in_each_format() {
        let request = SuggestRequest { system: "s".to_owned(), text: "t".to_owned(), image_jpeg: Some(vec![255, 216]) };
        let openai = openai_body("m", &request, true);
        assert_eq!(openai["messages"][1]["content"][1]["image_url"]["url"], "data:image/jpeg;base64,/9g=");
        assert_eq!(openai["response_format"]["json_schema"]["name"], "tags");
        assert!(openai_body("m", &request, false).get("response_format").is_none());
        let anthropic = anthropic_body("m", &request, true);
        assert!(anthropic_body("m", &request, false).get("output_config").is_none());
        assert_eq!(anthropic["messages"][0]["content"][0]["source"]["data"], "/9g=");
        assert_eq!(anthropic["system"], "s");
    }
}
