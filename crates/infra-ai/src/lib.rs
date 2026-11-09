//! AI adapters: an HTTP client for OpenAI-compatible servers and Anthropic, and the system keychain.

mod answer;
mod client;
mod keychain;

pub use client::HttpTagSuggester;
pub use keychain::Keychain;
