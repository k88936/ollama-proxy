use serde::{Deserialize, Serialize, Serializer};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Model {
    pub name: String,
    pub model: String,
    pub modified_at: Option<String>,
    pub size: Option<u64>,
    pub digest: Option<String>,
    pub details: Option<ModelDetails>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModelDetails {
    pub format: String,
    pub family: String,
    pub families: Vec<String>,
    pub parameter_size: String,
    pub quantization_level: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct ModelsResponse {
    pub models: Vec<Model>,
}

#[derive(Deserialize)]
pub struct GenerateRequest {
    pub model: String,
    pub prompt: String,
    pub stream: Option<bool>,
    pub options: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub stream: Option<bool>,
    pub options: Option<serde_json::Value>,
}
#[derive(Deserialize,Serialize)]
pub struct StreamChatChunk {
    pub model: String,
    pub message: Message,
    pub created_at: String,
    pub done: bool,
}


#[derive(Serialize)]
pub struct GenerateResponse {
    pub model: String,
    pub created_at: String,
    pub response: String,
    pub done: bool,
    pub context: Option<Vec<i32>>,
    pub total_duration: u64,
    pub load_duration: u64,
    pub prompt_eval_count: u64,
    pub eval_count: u64,
    pub eval_duration: u64,
}

#[derive(Serialize)]
pub struct ChatResponse {
    pub model: String,
    pub created_at: String,
    pub message: Message,
    pub done: bool,
    pub total_duration: u64,
    pub load_duration: u64,
    pub prompt_eval_count: u64,
    pub eval_count: u64,
    pub eval_duration: u64,
}
