use async_stream::stream;
use axum::routing::{get, post};
use futures_util::TryStreamExt;
// Make sure this is in scope
use std::path::Path;
use std::{env, fs};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{debug, info};
mod models;
mod providers;

use providers::Provider;
struct AppState {
    providers: Vec<Box<dyn Provider + Send + Sync>>,
}

use crate::models::{
    ApiType, ChatRequest, Config, GenerateRequest, GenerateResponse, Model, ModelsResponse,
};
use crate::providers::ollama_provider::OllamaProvider;
use crate::providers::openai_provider::OpenAIProvider;
use axum::{
    Router,
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use futures::StreamExt;
use std::sync::Arc;

/// Collects all content from a chat stream and concatenates it into a single string
async fn collect_content_from_stream(mut stream: providers::ChatChunkStream) -> Result<String, ()> {
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
pub fn map_model_name(provider_name: &String, model_name: &String) -> String {
    format!("[{}]-{}", provider_name, model_name)
}
async fn unmap_model(
    model_name: String,
    providers: &Vec<Box<dyn Provider + Send + Sync>>,
) -> (&Box<dyn Provider + Send + Sync>, String) {
    for provider in providers {
        let models = provider.get_models().await;
        if let Some(model) = models.iter().find(|m| m.model == model_name) {
            return (provider, model.name.clone());
        }
    }
    panic!(
        "Model '{}' not found in any provider, but that is impossible",
        model_name
    )
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
    debug!(
        "models: {}",
        models
            .iter()
            .map(|m| m.model.clone())
            .collect::<Vec<String>>()
            .join(",")
    );
    Ok(Json(ModelsResponse { models }))
}

async fn handle_generate(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<GenerateRequest>,
) -> Result<Json<GenerateResponse>, (StatusCode, String)> {
    // Create a simple message for chat
    let messages = vec![models::Message {
        role: "user".to_string(),
        content: payload.prompt.clone(),
    }];

    // Use the provider's chat_stream method to generate a response
    let (provider, model) = unmap_model(payload.model, &state.providers).await;

    let stream = match provider.chat(&model, &messages, payload.options.clone()) {
        Ok(stream) => stream,
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to generate response".to_string(),
            ));
        }
    };

    // Collect all chunks from the stream and concatenate content
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

    debug!(
        "\n<<< generate: {{{}}} \n>>> response: {{{}}}",
        payload.prompt, resp.response
    );
    Ok(Json(resp))
}

async fn handle_chat(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChatRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Use streaming method for both streaming and non-streaming requests
    let (provider, model) = unmap_model(payload.model, &state.providers).await;

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
        // Non-streaming: collect all chunks from a stream and concatenate content
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
            message: models::Message {
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

        // Log chat similar to generate: last user message and response
        let last_user_message = payload
            .messages
            .iter()
            .rfind(|m| m.role == "user")
            .map(|m| m.content.clone())
            .unwrap_or_default();
        debug!(
            "\n<<< chat: {{{}}} \n>>> response {{{}}}",
            last_user_message, resp.message.content
        );

        Ok(Json(resp).into_response())
    } else {
        // Streaming mode with logging similar to generate
        let last_user_message = payload
            .messages
            .iter()
            .rfind(|m| m.role == "user")
            .map(|m| m.content.clone())
            .unwrap_or_default();
        let user_for_log = last_user_message.clone();

        let wrapped_stream = stream! {
            let mut acc = String::new();
            let mut s = stream;
            while let Some(item) = s.next().await {
                if let Ok(chunk) = &item {
                    if !chunk.done {
                        acc.push_str(&chunk.message.content);
                    }
                }
                yield item;
            }
            debug!("\n<<< chat(stream): {{{}}} \n>>> response {{{}}}", user_for_log, acc);
        };

        Ok((
            [(
                axum::http::header::CONTENT_TYPE,
                "application/x-ndjson".to_string(),
            )],
            axum::body::Body::from_stream(
                wrapped_stream
                    .map(|obj| serde_json::to_string(&obj.unwrap())) // This returns Result<String, _>
                    .map_ok(|s| format!("{}\n", s)), // Transform Ok(String) -> Ok(String + \n)
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
#[tokio::main]
async fn main() {
    // 初始化日志记录
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
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

    // 从配置文件加载
    let config_file = fs::File::open(&config_path).expect("Failed to open config file");
    let config: Config = serde_yaml::from_reader(config_file).unwrap();

    let state = AppState {
        providers: load_providers(&config),
    };
    let state = Arc::new(state);
    let app: Router = Router::new()
        .route("/", get(handle_status))
        .route("/api/tags", get(handle_tags))
        .route("/api/generate", post(handle_generate))
        .route("/api/chat", post(handle_chat))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .fallback(not_found)
        .with_state(state);
    // we should not allow lan for security's sake
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", config.port))
        .await
        .unwrap();
    info!(
        "Ollama API server listening on http://127.0.0.1:{}",
        config.port
    );

    axum::serve(listener, app).await.unwrap();
}

fn load_providers(config: &Config) -> Vec<Box<dyn Provider + Send + Sync>> {
    let providers = config
        .providers
        .iter()
        .map(|item| {
            let secret = if let Some(secret) = &item.secret {
                secret.clone()
            } else {
                "".to_string()
            };
            let models = item.models.clone().unwrap_or_default();
            let models = models
                .iter()
                .map(|model| Model {
                    name: model.clone(),
                    model: map_model_name(&item.name, model),
                    modified_at: None,
                    size: None,
                    digest: None,
                    details: None,
                })
                .collect();
            let provider: Box<dyn Provider + Send + Sync> = match item.api_type {
                ApiType::Ollama => Box::new(OllamaProvider::new(item.url.clone(), secret, models)),
                ApiType::Openai => Box::new(OpenAIProvider::new(item.url.clone(), secret, models)),
            };
            provider
        })
        .collect();
    providers
}

fn get_config_path() -> std::path::PathBuf {
    let file_name = "ollama-proxy.yaml";
    // 尝试获取 HOME 目录 (Unix/Linux/macOS)
    for env_name in vec!["HOME", "USERPROFILE"] {
        if let Ok(home_dir) = env::var(env_name) {
            return Path::new(&home_dir).join(file_name);
        }
    }
    panic!("cant get user home by env: HOME/USERPROFILE");
}
