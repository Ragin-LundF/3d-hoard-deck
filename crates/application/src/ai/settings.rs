use serde::{Deserialize, Serialize};

/// Who answers. Everything but Anthropic speaks the `OpenAI` chat completions API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiProvider {
    /// No AI: the app shows no AI feature and sends nothing anywhere.
    #[default]
    Off,
    LmStudio,
    Ollama,
    OpenAi,
    Anthropic,
    /// Any other OpenAI-compatible server.
    Custom,
}

impl AiProvider {
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::LmStudio => "lmStudio",
            Self::Ollama => "ollama",
            Self::OpenAi => "openAi",
            Self::Anthropic => "anthropic",
            Self::Custom => "custom",
        }
    }

    /// The address used when the settings leave it empty.
    #[must_use]
    pub fn default_url(self) -> &'static str {
        match self {
            Self::LmStudio => "http://localhost:1234/v1",
            Self::Ollama => "http://localhost:11434/v1",
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::Off | Self::Custom => "",
        }
    }

    /// The model used when the settings name none and the server does not list one.
    #[must_use]
    pub fn default_model(self) -> Option<&'static str> {
        match self {
            // Small and cheap is right for one short answer per picture.
            Self::Anthropic => Some("claude-haiku-5-5"),
            _ => None,
        }
    }

    /// Environment variable read when no key is stored, as in the DMS.
    #[must_use]
    pub fn key_variable(self) -> Option<&'static str> {
        match self {
            Self::OpenAi => Some("OPENAI_API_KEY"),
            Self::Anthropic => Some("ANTHROPIC_API_KEY"),
            _ => None,
        }
    }

    /// Runs on another computer, so pictures and names leave this one.
    #[must_use]
    pub fn is_remote(self, url: &str) -> bool {
        match self {
            Self::OpenAi | Self::Anthropic => true,
            Self::Custom => !["localhost", "127.0.0.1", "[::1]"].iter().any(|host| url.contains(host)),
            _ => false,
        }
    }
}

/// AI settings, per index (one user, one computer). The API key is never part of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AiSettings {
    pub provider: AiProvider,
    /// Empty: [`AiProvider::default_url`].
    pub base_url: String,
    /// Empty: the first chat model the server lists, else [`AiProvider::default_model`].
    pub model: String,
    pub send_image: bool,
    pub send_path: bool,
    pub max_tags: u8,
    pub timeout_seconds: u16,
    /// Replaces the built-in instructions; `{name}`, `{path}`, `{tags}` and `{max}` are filled in.
    pub prompt: String,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            provider: AiProvider::Off,
            base_url: String::new(),
            model: String::new(),
            send_image: true,
            send_path: true,
            max_tags: 3,
            timeout_seconds: 120,
            prompt: String::new(),
        }
    }
}

impl AiSettings {
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.provider != AiProvider::Off
    }

    #[must_use]
    pub fn url(&self) -> String {
        let url = if self.base_url.trim().is_empty() { self.provider.default_url() } else { self.base_url.trim() };
        url.trim_end_matches('/').to_owned()
    }
}
