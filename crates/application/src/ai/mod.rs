//! Optional tag suggestions from an AI (a local server such as LM Studio, or a cloud service).

mod prompt;
mod settings;
mod suggest_tags;

pub use settings::{AiProvider, AiSettings};
pub use suggest_tags::{AiTest, AiView, KeyChange, SuggestTags};
