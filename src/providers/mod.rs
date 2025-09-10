pub mod openai_provider;

use crate::models::{Message, Model, StreamChatChunk};
use serde_json::Value;

#[derive(Debug)]
pub struct ProviderError {
    pub message: String,
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Provider error: {}", self.message)
    }
}

impl std::error::Error for ProviderError {}

// 为Box<dyn Provider>实现Clone trait

// 定义可克隆的 Provider trait
#[async_trait::async_trait]
pub trait Provider {
    async fn chat(
        &self,
        model: &String,
        messages: &[Message],
        option: Option<Value>,
    ) -> Result<String, ProviderError>;

    fn chat_stream(
        &self,
        model: &String,
        messages: &[Message],
        option: Option<Value>,
    ) -> Result<ChatChunkStream, ProviderError>;

    async fn get_models(&self) -> Result<Vec<Model>, ProviderError>;
}

use futures::Stream;
use std::pin::Pin;
// 定义ChatChunkStream类型用于处理聊天流

pub type ChatChunkStream = Pin<Box<dyn Stream<Item = Result<StreamChatChunk, ProviderError>> + Send>>;