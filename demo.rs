use axum::{
    extract::{Json, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;

#[derive(Clone)]
struct AppState {
    provider: FakeProvider,
}

#[derive(Serialize, Deserialize, Clone)]
struct Model {
    name: String,
    model: String,
    modified_at: String,
    size: u64,
    digest: String,
    details: ModelDetails,
}

#[derive(Serialize, Deserialize, Clone)]
struct ModelDetails {
    format: String,
    family: String,
    families: Vec<String>,
    parameter_size: String,
    quantization_level: String,
}

#[derive(Deserialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: Option<bool>,
    options: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: Option<bool>,
    options: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct GenerateResponse {
    model: String,
    created_at: String,
    response: String,
    done: bool,
    context: Option<Vec<i32>>,
    total_duration: u64,
    load_duration: u64,
    prompt_eval_count: u64,
    eval_count: u64,
    eval_duration: u64,
}

#[derive(Serialize)]
struct ChatResponse {
    model: String,
    created_at: String,
    message: Message,
    done: bool,
    total_duration: u64,
    load_duration: u64,
    prompt_eval_count: u64,
    eval_count: u64,
    eval_duration: u64,
}

// 定义ChatChunkStream类型用于处理聊天流
struct ChatChunkStream {
    inner: std::pin::Pin<Box<dyn futures::Stream<Item = Result<String, std::convert::Infallible>> + Send>>,
}

impl ChatChunkStream {
    fn new(model: String, chunks: Vec<String>) -> Self {
        let stream = futures::stream::iter(chunks.into_iter().enumerate().map(move |(i, chunk)| {
            let is_last = i == 9; // 最后一个chunk
            let model_clone = model.clone();
            let resp = ChatResponse {
                model: model_clone,
                created_at: chrono::Utc::now().to_rfc3339(),
                message: Message {
                    role: "assistant".to_string(),
                    content: chunk,
                },
                done: is_last,
                total_duration: if is_last { 1_234_567_890 } else { 0 },
                load_duration: if i == 0 { 123_456_789 } else { 0 },
                prompt_eval_count: if i == 0 { 10 } else { 0 },
                eval_count: i as u64 + 1,
                eval_duration: if is_last { 987_654_321 } else { 0 },
            };
            Ok::<_, std::convert::Infallible>(serde_json::to_string(&resp).unwrap() + "\n")
        }));
        
        Self {
            inner: Box::pin(stream),
        }
    }
}

impl futures::Stream for ChatChunkStream {
    type Item = Result<String, std::convert::Infallible>;
    
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }
}

#[derive(Serialize)]
struct ModelsResponse {
    models: Vec<Model>,
}

#[derive(Debug)]
struct ProviderError {
    message: String,
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Provider error: {}", self.message)
    }
}

impl std::error::Error for ProviderError {}

trait Provider {
    type Model;
    type Error;
    
    fn chat(
        &self,
        model: &Self::Model,
        messages: &[Message],
        option: Option<serde_json::Value>,
    ) -> Result<String, Self::Error>;
    
    fn chat_stream(
        &self,
        model: &Self::Model,
        messages: &[Message],
        option: Option<serde_json::Value>,
    ) -> Result<ChatChunkStream, Self::Error>;
    
    fn get_models(&self) -> Result<Vec<Self::Model>, Self::Error>;
}

// FakeProvider implementation
#[derive(Clone)]
struct FakeProvider {
    models: Vec<Model>,
}

impl FakeProvider {
    fn new() -> Self {
        let models = vec![
            Model {
                name: "llama3".to_string(),
                model: "llama3".to_string(),
                modified_at: "2024-04-01T00:00:00Z".to_string(),
                size: 4820852800,
                digest: "sha256:1234567890abcdef...".to_string(),
                details: ModelDetails {
                    format: "gguf".to_string(),
                    family: "llama".to_string(),
                    families: vec!["llama".to_string()],
                    parameter_size: "8B".to_string(),
                    quantization_level: "Q4_K_M".to_string(),
                },
            },
        ];
        
        Self { models }
    }
}

impl Provider for FakeProvider {
    type Model = Model;
    type Error = ProviderError;
    
    fn chat(
        &self,
        model: &Self::Model,
        messages: &[Message],
        _option: Option<serde_json::Value>,
    ) -> Result<String, Self::Error> {
        // Check if model exists
        if !self.models.iter().any(|m| m.name == model.name) {
            return Err(ProviderError {
                message: "Model not found".to_string(),
            });
        }
        
        // Get last message content
        let last_msg = messages
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default();
            
        Ok(format!("Replied to: {}", last_msg))
    }
    
    fn chat_stream(
        &self,
        model: &Self::Model,
        messages: &[Message],
        _option: Option<serde_json::Value>,
    ) -> Result<ChatChunkStream, Self::Error> {
        // Check if model exists
        if !self.models.iter().any(|m| m.name == model.name) {
            return Err(ProviderError {
                message: "Model not found".to_string(),
            });
        }
        
        // Get last message content
        let last_msg = messages
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default();
            
        let chunks: Vec<String> = vec![
            "Hello".to_string(),
            ", this".to_string(),
            " is".to_string(),
            " a".to_string(),
            " simulated".to_string(),
            " stream".to_string(),
            " reply".to_string(),
            " to".to_string(),
            format!(" '{}'", last_msg),
            ".".to_string(),
        ];
        
        Ok(ChatChunkStream::new(model.name.clone(), chunks))
    }
    
    fn get_models(&self) -> Result<Vec<Self::Model>, Self::Error> {
        Ok(self.models.clone())
    }
}

// 添加一个中间件来记录所有请求的详细信息
async fn log_request_middleware(
    request: Request<axum::body::Body>, 
    next: Next
) -> impl IntoResponse {
    // 记录请求的详细信息
    info!("=== Incoming Request ===");
    info!("Method: {}", request.method());
    info!("URI: {}", request.uri());
    info!("Version: {:?}", request.version());
    info!("Headers:");
    for (name, value) in request.headers() {
        info!("  {}: {:?}", name, value);
    }
    
    // 继续处理请求
    let response = next.run(request).await;
    
    // 记录响应信息
    info!("=== Response ===");
    info!("Status: {}", response.status());
    info!("Headers:");
    for (name, value) in response.headers() {
        info!("  {}: {:?}", name, value);
    }
    
    response
}

// 添加处理未匹配路由的函数
async fn not_found() -> (StatusCode, String) {
    info!("=== Unmatched Route Request ===");
    (StatusCode::NOT_FOUND, "Endpoint not found".to_string())
}

async fn get_models(State(state): State<Arc<AppState>>) -> Result<Json<ModelsResponse>, (StatusCode, String)> {
    match state.provider.get_models() {
        Ok(models) => Ok(Json(ModelsResponse { models })),
        Err(_) => Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to get models".to_string())),
    }
}

async fn generate(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<GenerateRequest>,
) -> Result<Json<GenerateResponse>, (StatusCode, String)> {
    // Find the model
    let model = match state.provider.get_models() {
        Ok(models) => {
            match models.into_iter().find(|m| m.name == payload.model) {
                Some(model) => model,
                None => return Err((StatusCode::BAD_REQUEST, "Model not found".to_string())),
            }
        },
        Err(_) => return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to get models".to_string())),
    };
    
    // Use the provider to generate response
    let fake_response = match state.provider.chat(
        &model,
        &[Message {
            role: "user".to_string(),
            content: payload.prompt.clone(),
        }],
        payload.options.clone(),
    ) {
        Ok(response) => response,
        Err(_) => return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to generate response".to_string())),
    };

    let resp = GenerateResponse {
        model: payload.model,
        created_at: chrono::Utc::now().to_rfc3339(),
        response: fake_response,
        done: true,
        context: Some(vec![1, 2, 3]),
        total_duration: 1_234_567_890,
        load_duration: 123_456_789,
        prompt_eval_count: 10,
        eval_count: 25,
        eval_duration: 987_654_321,
    };

    Ok(Json(resp))
}

async fn chat(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChatRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Find the model
    let model = match state.provider.get_models() {
        Ok(models) => {
            match models.into_iter().find(|m| m.name == payload.model) {
                Some(model) => model,
                None => return Err((StatusCode::BAD_REQUEST, "Model not found".to_string())),
            }
        },
        Err(_) => return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to get models".to_string())),
    };
    
    let stream_mode = payload.stream.unwrap_or(true);
    if !stream_mode {
        // Non-streaming: return final response
        let response = match state.provider.chat(
            &model,
            &payload.messages,
            payload.options.clone(),
        ) {
            Ok(response) => response,
            Err(_) => return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to generate response".to_string())),
        };

        let resp = ChatResponse {
            model: payload.model,
            created_at: chrono::Utc::now().to_rfc3339(),
            message: Message {
                role: "assistant".to_string(),
                content: response,
            },
            done: true,
            total_duration: 1_234_567_890,
            load_duration: 123_456_789,
            prompt_eval_count: 10,
            eval_count: 25,
            eval_duration: 987_654_321,
        };

        return Ok(Json(resp).into_response());
    }

    // Streaming mode: use provider's chat_stream method
    let stream = match state.provider.chat_stream(
        &model,
        &payload.messages,
        payload.options.clone(),
    ) {
        Ok(stream) => stream,
        Err(_) => return Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to generate stream response".to_string())),
    };

    Ok((
        [(
            axum::http::header::CONTENT_TYPE,
            "application/x-ndjson".to_string(),
        )],
        axum::body::Body::from_stream(stream),
    )
        .into_response())
}

async fn root(
    State(_state): State<Arc<AppState>>,
) -> Result<String, (StatusCode, String)> {
    Ok("Ollama is running".to_string())
}

#[tokio::main]
async fn main() {
    // 初始化日志记录
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
        
    info!("Starting fake Ollama API server...");

    let provider = FakeProvider::new();
    let state = Arc::new(AppState { provider });

    let app: Router = Router::new()
        .route("/", get(root))
        .route("/api/tags", get(get_models))
        .route("/api/generate", post(generate))
        .route("/api/chat", post(chat))
        .layer(CorsLayer::permissive())
        .fallback(not_found) // 处理未匹配的路由
        .with_state(state)
        .layer(axum::middleware::from_fn(log_request_middleware)); // 应用日志中间件到所有路由

    let listener = tokio::net::TcpListener::bind("127.0.0.1:11434")
        .await
        .unwrap();
    info!("🚀 Fake Ollama API server listening on http://localhost:11434");

    axum::serve(listener, app).await.unwrap();
}
