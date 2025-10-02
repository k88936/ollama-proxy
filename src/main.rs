use axum::middleware::Next;
use axum::routing::{get, post};
use futures_util::TryStreamExt;
// Make sure this is in scope
use std::path::Path;
use std::{env, fs};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;
mod models;
mod providers;

use providers::Provider;
struct AppState {
    providers: Vec<Box<dyn Provider + Send + Sync>>,
}

use crate::models::{
    ApiType, ChatRequest, GenerateRequest, GenerateResponse, Model, ModelsResponse,
};
use crate::providers::ollama_provider::OllamaProvider;
use crate::providers::openai_provider::OpenAIProvider;
use axum::http::Request;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    Router,
};
use futures::StreamExt;
use indoc::indoc;
use std::sync::Arc;

/// Collects all content from a chat stream and concatenates it into a single string
async fn collect_content_from_stream(
    mut stream: crate::providers::ChatChunkStream,
) -> Result<String, ()> {
    let mut content = String::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                if !chunk.done {
                    content.push_str(&chunk.message.content);
                }
            }
            Err(_) => return Err(()),
        }
    }

    Ok(content)
}
async fn get_provider_by_model(
    model_name: String,
    providers: &Vec<Box<dyn Provider + Send + Sync>>,
) -> (&Box<dyn Provider + Send + Sync>, String) {
    for provider in providers {
        let models = provider.get_models().await;
        if let Some(model) = models.iter().find(|m| m.model == model_name) {
            return (provider, model.name.clone());
        }
    }
    panic!("Model '{}' not found in any provider", model_name)
}
async fn handle_status(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    "Ollama is running".to_string()
}

async fn handle_tags(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ModelsResponse>, (StatusCode, String)> {
    // Collect all models from providers
    let mut models: Vec<Model> = Vec::new();
    for provider in &state.providers {
        let mut provider_models = provider.get_models().await;
        models.append(&mut provider_models);
    }

    Ok(Json(ModelsResponse { models }))
}

async fn handle_generate(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<GenerateRequest>,
) -> Result<Json<GenerateResponse>, (StatusCode, String)> {
    // Create a simple message for chat
    let messages = vec![crate::models::Message {
        role: "user".to_string(),
        content: payload.prompt.clone(),
    }];

    // Use the provider's chat_stream method to generate response
    let (provider, model) = get_provider_by_model(payload.model, &state.providers).await;

    let stream = match provider.chat(&model, &messages, payload.options.clone()) {
        Ok(stream) => stream,
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to generate response".to_string(),
            ));
        }
    };

    // Collect all chunks from stream and concatenate content
    let content = match collect_content_from_stream(stream).await {
        Ok(content) => content,
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to generate response".to_string(),
            ));
        }
    };

    let resp = GenerateResponse {
        model,
        created_at: chrono::Utc::now().to_rfc3339(),
        response: content,
        done: true,
        context: None,
        total_duration: 0,
        load_duration: 0,
        prompt_eval_count: 0,
        eval_count: 0,
        eval_duration: 0,
    };

    Ok(Json(resp))
}

async fn handle_chat(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChatRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Use streaming method for both streaming and non-streaming requests
    let (provider, model) = get_provider_by_model(payload.model, &state.providers).await;

    let stream = match provider.chat(&model, &payload.messages, payload.options.clone()) {
        Ok(stream) => stream,
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to generate response".to_string(),
            ));
        }
    };

    let stream_mode = payload.stream.unwrap_or(true);
    if !stream_mode {
        // Non-streaming: collect all chunks from stream and concatenate content
        let content = match collect_content_from_stream(stream).await {
            Ok(content) => content,
            Err(_) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to generate response".to_string(),
                ));
            }
        };

        let resp = models::ChatResponse {
            model,
            created_at: chrono::Utc::now().to_rfc3339(),
            message: crate::models::Message {
                role: "assistant".to_string(),
                content,
            },
            done: true,
            total_duration: 0,
            load_duration: 0,
            prompt_eval_count: 0,
            eval_count: 0,
            eval_duration: 0,
        };

        Ok(Json(resp).into_response())
    } else {
        // Streaming mode: return stream as before
        Ok((
            [(
                axum::http::header::CONTENT_TYPE,
                "application/x-ndjson".to_string(),
            )],
            axum::body::Body::from_stream(
                stream
                    .map(|obj| serde_json::to_string(&obj.unwrap())) // This returns Result<String, _>
                    .map_ok(|s| format!("{}\n", s)), // ✅ Transform Ok(String) -> Ok(String + \n)
            ),
        )
            .into_response())
    }
}
// 处理未匹配路由的函数
async fn not_found() -> (StatusCode, String) {
    info!("=== Unmatched Route Request ===");
    (StatusCode::NOT_FOUND, "Endpoint not found".to_string())
}

// 一个中间件: 记录所有请求的详细信息
async fn log_request_middleware(
    request: Request<axum::body::Body>,
    next: Next,
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

#[tokio::main]
async fn main() {
    // 初始化日志记录
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting Ollama API server...");
    // 获取配置文件路径，尝试多种可能的路径
    let config_path = get_config_path();

    // 如果配置文件不存在，创建示例配置文件
    if !config_path.exists() {
        let example_config = models::get_config_demo();
        fs::write(&config_path, example_config).unwrap();
        info!("已创建示例配置文件: {:?}", config_path);
        return;
    }

    // 从配置文件加载环境变量
    let config_file = fs::File::open(&config_path).expect("Failed to open config file");
    let config: models::Config = serde_yaml::from_reader(config_file).unwrap();

    let providers = config
        .items
        .iter()
        .map(|item| {
            let secret = if let Some(secret) = &item.secret {
                secret.clone()
            } else {
                "".to_string()
            };
            let models = item.models.clone().unwrap_or_default();
            let provider: Box<dyn Provider + Send + Sync> = match item.api_type {
                ApiType::Ollama => Box::new(OllamaProvider::new(
                    item.name.clone(),
                    item.url.clone(),
                    secret,
                    models,
                )),
                ApiType::Openai => {
                    Box::new(OpenAIProvider::new(
                        item.name.clone(),
                        secret,
                        item.url.clone(),
                        models,
                    ))
                }
            };
            provider
        })
        .collect();

    let state = AppState { providers };
    let state = Arc::new(state);
    let app: Router = Router::new()
        .route("/", get(handle_status))
        .route("/api/tags", get(handle_tags))
        .route("/api/generate", post(handle_generate))
        .route("/api/chat", post(handle_chat))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .fallback(not_found) // 处理未匹配的路由
        .with_state(state)
        .layer(axum::middleware::from_fn(log_request_middleware)); // 应用日志中间件到所有路由

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", config.port))
        .await
        .unwrap();
    info!("Ollama API server listening on http://localhost:11434");

    axum::serve(listener, app).await.unwrap();
}
fn get_config_path() -> std::path::PathBuf {
    let file_name = "ollama-proxy.yaml";
    // 尝试获取 HOME 目录 (Unix/Linux/macOS)
    if let Ok(home_dir) = env::var("HOME") {
        return Path::new(&home_dir).join(file_name);
    }

    // 尝试获取 USERPROFILE 目录 (Windows)
    if let Ok(home_dir) = env::var("USERPROFILE") {
        return Path::new(&home_dir).join(file_name);
    }
    panic!("cant get user home")
}
