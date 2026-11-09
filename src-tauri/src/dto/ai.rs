use hoarddeck_application::ai::{AiFolders, AiProvider, AiSettings, AiTest, AiView, Resolution, ReviewItem};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AiProviderDto {
    Off,
    LmStudio,
    Ollama,
    OpenAi,
    Anthropic,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AiSettingsDto {
    pub provider: AiProviderDto,
    pub base_url: String,
    pub model: String,
    pub send_image: bool,
    pub send_path: bool,
    pub max_tags: u8,
    pub timeout_seconds: u16,
    pub prompt: String,
}

/// The settings plus what the UI may know about the key: whether one is stored, never the key itself.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AiViewDto {
    /// AI is on when `settings.provider` is not `off`.
    pub settings: AiSettingsDto,
    pub has_key: bool,
    pub key_from_environment: bool,
    /// Pictures and names go to another computer.
    pub remote: bool,
    /// The address used when the field is empty.
    pub default_url: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AiTestDto {
    pub models: Vec<String>,
    pub model: String,
    pub tags: Vec<String>,
    pub millis: u32,
}

impl From<AiProvider> for AiProviderDto {
    fn from(provider: AiProvider) -> Self {
        match provider {
            AiProvider::Off => Self::Off,
            AiProvider::LmStudio => Self::LmStudio,
            AiProvider::Ollama => Self::Ollama,
            AiProvider::OpenAi => Self::OpenAi,
            AiProvider::Anthropic => Self::Anthropic,
            AiProvider::Custom => Self::Custom,
        }
    }
}

impl From<AiProviderDto> for AiProvider {
    fn from(provider: AiProviderDto) -> Self {
        match provider {
            AiProviderDto::Off => Self::Off,
            AiProviderDto::LmStudio => Self::LmStudio,
            AiProviderDto::Ollama => Self::Ollama,
            AiProviderDto::OpenAi => Self::OpenAi,
            AiProviderDto::Anthropic => Self::Anthropic,
            AiProviderDto::Custom => Self::Custom,
        }
    }
}

impl From<AiSettings> for AiSettingsDto {
    fn from(settings: AiSettings) -> Self {
        Self {
            provider: settings.provider.into(),
            base_url: settings.base_url,
            model: settings.model,
            send_image: settings.send_image,
            send_path: settings.send_path,
            max_tags: settings.max_tags,
            timeout_seconds: settings.timeout_seconds,
            prompt: settings.prompt,
        }
    }
}

impl From<AiSettingsDto> for AiSettings {
    fn from(dto: AiSettingsDto) -> Self {
        Self {
            provider: dto.provider.into(),
            base_url: dto.base_url,
            model: dto.model,
            send_image: dto.send_image,
            send_path: dto.send_path,
            max_tags: dto.max_tags,
            timeout_seconds: dto.timeout_seconds,
            prompt: dto.prompt,
        }
    }
}

impl From<AiView> for AiViewDto {
    fn from(view: AiView) -> Self {
        Self {
            default_url: view.settings.provider.default_url().to_owned(),
            settings: view.settings.into(),
            has_key: view.has_key,
            key_from_environment: view.key_from_environment,
            remote: view.remote,
        }
    }
}

impl From<AiTest> for AiTestDto {
    fn from(test: AiTest) -> Self {
        Self {
            models: test.models,
            model: test.model,
            tags: test.tags,
            millis: u32::try_from(test.millis).unwrap_or(u32::MAX),
        }
    }
}

/// A model with suggested tags waiting for review.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewItemDto {
    pub id: u32,
    pub title: String,
    pub path: String,
    pub cover: Option<String>,
    pub suggested: Vec<String>,
    pub own: Vec<String>,
}

impl From<ReviewItem> for ReviewItemDto {
    fn from(item: ReviewItem) -> Self {
        Self {
            id: u32::try_from(item.id.0).unwrap_or(u32::MAX),
            title: item.title,
            path: item.path.into(),
            cover: item.cover.map(String::from),
            suggested: item.suggested.into_iter().map(String::from).collect(),
            own: item.own.into_iter().map(String::from).collect(),
        }
    }
}

/// Suggested tags to keep and to drop on one model.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DecisionDto {
    pub id: u32,
    pub keep: Vec<String>,
    pub dismiss: Vec<String>,
}

/// What a decision changed; sent back as is to undo it.
#[derive(Debug, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionDto {
    pub path: String,
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

impl From<Resolution> for ResolutionDto {
    fn from(resolution: Resolution) -> Self {
        Self {
            path: resolution.path.into(),
            added: resolution.added.into_iter().map(String::from).collect(),
            removed: resolution.removed.into_iter().map(String::from).collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AiFoldersDto {
    pub included: Vec<String>,
    pub excluded: Vec<String>,
    pub only_untagged: bool,
    pub after_scan: bool,
    pub skip_without_preview: bool,
}

impl From<AiFolders> for AiFoldersDto {
    fn from(folders: AiFolders) -> Self {
        Self {
            included: folders.included.into_iter().map(String::from).collect(),
            excluded: folders.excluded.into_iter().map(String::from).collect(),
            only_untagged: folders.only_untagged,
            after_scan: folders.after_scan,
            skip_without_preview: folders.skip_without_preview,
        }
    }
}
