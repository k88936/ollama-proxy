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
    provider: Box<dyn Provider + Send + Sync>,
}

use crate::models::{ChatRequest, GenerateRequest, GenerateResponse, ModelsResponse};
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
use serde_json::to_string;
use std::sync::Arc;

async fn handle_status(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    "Ollama is running".to_string()
}

async fn handle_tags(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ModelsResponse>, (StatusCode, String)> {
    // Lock the state for mutation
    let models = state.provider.get_models().await.unwrap();
    Ok(Json(ModelsResponse { models }))
}

async fn handle_generate(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<GenerateRequest>,
) -> Result<Json<GenerateResponse>, (StatusCode, String)> {
    // Find the model

    // Create a simple message for chat
    let messages = vec![crate::models::Message {
        role: "user".to_string(),
        content: payload.prompt.clone(),
    }];

    // Use the provider's chat method to generate response
    let response = match state
        .provider
        .chat(&payload.model, &messages, payload.options.clone())
        .await
    {
        Ok(response) => response,
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to generate response".to_string(),
            ));
        }
    };

    let resp = GenerateResponse {
        model: payload.model,
        created_at: chrono::Utc::now().to_rfc3339(),
        response,
        done: true,
        context: Some(vec![1, 2, 3]),
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
    // Find the model

    let stream_mode = payload.stream.unwrap_or(true);
    if !stream_mode {
        // Non-streaming: return final response
        let response = match state
            .provider
            .chat(&payload.model, &payload.messages, payload.options.clone())
            .await
        {
            Ok(response) => response,
            Err(_) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to generate response".to_string(),
                ));
            }
        };

        let resp = models::ChatResponse {
            model: payload.model,
            created_at: chrono::Utc::now().to_rfc3339(),
            message: crate::models::Message {
                role: "assistant".to_string(),
                content: response,
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
        // Streaming mode: use provider's chat_stream method
        let stream = match state.provider.chat_stream(
            &payload.model,
            &payload.messages,
            payload.options.clone(),
        ) {
            Ok(stream) => stream,
            Err(_) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to generate stream response".to_string(),
                ));
            }
        };

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

    info!("Starting fake Ollama API server...");
    // 获取配置文件路径，尝试多种可能的路径
    let config_path = get_config_path();

    // 如果配置文件不存在，创建示例配置文件
    if !config_path.exists() {
        let example_config = indoc! { r#"
            # USE=ollama
            # #Remote Ollama Proxy Configuration
            # OLLAMA_USER=your_username
            # OLLAMA_PASS=your_password
            # OLLAMA_BASE_URL=https://api.example.com
            #
            # USE=openai
            # OPENAI_API_KEY=your_api_key
            # OPENAI_BASE_URL=https://api.example.com
            # MODELS=qwen3-coder-plus,qwen3
            "#};
        fs::write(&config_path, example_config).unwrap();
        info!("已创建示例配置文件: {:?}", config_path);
        return;
    }

    // 从配置文件加载环境变量
    dotenvy::from_filename(config_path.to_str().unwrap()).ok();

    let provider_use = env::var("USE").unwrap();
    let provider: Box<dyn Provider + Send + Sync> = match provider_use.as_str() {
        // "ollama" => {
        //     // 获取环境变量中的认证信息
        //     let user = env::var("OLLAMA_USER")?;
        //     let password = env::var("OLLAMA_PASS")?;
        //     let url = env::var("OLLAMA_BASE_URL")?;
        //     OllamaProvider::new()
        // }
        "openai" => {
            let api_key = env::var("OPENAI_API_KEY").unwrap();
            let url = env::var("OPENAI_BASE_URL").unwrap();
            let models = env::var("MODELS")
                .unwrap()
                .split(',')
                .map(|s| s.trim().to_string())
                .collect::<Vec<String>>();
            Box::new(OpenAIProvider::new(api_key, url, models))
        }
        _ => panic!("provider not found"),
    };

    let state = AppState { provider };
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

    let listener = tokio::net::TcpListener::bind("127.0.0.1:11434")
        .await
        .unwrap();
    info!("🚀 Fake Ollama API server listening on http://localhost:11434");

    axum::serve(listener, app).await.unwrap();
}
fn get_config_path() -> std::path::PathBuf {
    // 尝试获取 HOME 目录 (Unix/Linux/macOS)
    if let Ok(home_dir) = env::var("HOME") {
        return Path::new(&home_dir).join(".ollama-proxy");
    }

    // 尝试获取 USERPROFILE 目录 (Windows)
    if let Ok(home_dir) = env::var("USERPROFILE") {
        return Path::new(&home_dir).join(".ollama-proxy");
    }
    panic!("cant get user home")
}
