//! Optional tag suggestions from an AI (a local server such as LM Studio, or a cloud service).

mod folders;
mod prompt;
mod review;
mod settings;
mod suggest_tags;

pub use folders::AiFolders;
pub use review::{Decision, Resolution, ReviewItem};
pub use settings::{AiProvider, AiSettings};
pub use suggest_tags::{AiTest, AiView, KeyChange, SuggestTags};
