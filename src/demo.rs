use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use futures::stream;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    models: Vec<Model>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Model {
    name: String,
    // modified_at: String,
    // size: u64,
    // digest: String,
    // details: ModelDetails,
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

#[derive(Serialize)]
struct ModelsResponse {
    models: Vec<Model>,
}

async fn get_models(State(state): State<Arc<AppState>>) -> Json<ModelsResponse> {
    Json(ModelsResponse {
        models: state.models.clone(),
    })
}

async fn generate(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<GenerateRequest>,
) -> Result<Json<GenerateResponse>, (StatusCode, String)> {
    if !state.models.iter().any(|m| m.name == payload.model) {
        return Err((StatusCode::BAD_REQUEST, "Model not found".to_string()));
    }

    let fake_response = format!("Echo: {}", payload.prompt);

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
    if !state.models.iter().any(|m| m.name == payload.model) {
        return Err((StatusCode::BAD_REQUEST, "Model not found".to_string()));
    }

    let stream_mode = payload.stream.unwrap_or(true);

    if !stream_mode {
        // Non-streaming: return final response
        let last_msg = payload
            .messages
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default();

        let resp = ChatResponse {
            model: payload.model,
            created_at: chrono::Utc::now().to_rfc3339(),
            message: Message {
                role: "assistant".to_string(),
                content: format!("Replied to: {}", last_msg),
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

    // Streaming mode: simulate chunks
    let model = payload.model.clone();
    let last_msg = payload
        .messages
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

    let stream = stream::iter(chunks.into_iter().enumerate().map(move |(i, chunk)| {
        let is_last = i == 8; // adjust based on chunks len - 1
        let resp = ChatResponse {
            model: model.clone(),
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

    Ok((
        [(
            axum::http::header::CONTENT_TYPE,
            "application/x-ndjson".to_string(),
        )],
        axum::body::Body::from_stream(stream),
    )
        .into_response())
}
async fn health(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<GenerateRequest>,
) -> Result<String, (StatusCode, String)> {

    Ok("Ollama is running".to_string())
}
#[tokio::main]
async fn main() {
    let models = vec![
        Model {
            name: "llama3".to_string(),
            // modified_at: "2024-04-01T00:00:00Z".to_string(),
            // size: 4820852800,
            // digest: "sha256:1234567890abcdef...".to_string(),
            // details: ModelDetails {
            //     format: "gguf".to_string(),
            //     family: "llama".to_string(),
            //     families: vec!["llama".to_string()],
            //     parameter_size: "8B".to_string(),
            //     quantization_level: "Q4_K_M".to_string(),
            // },
        },
    ];

    let state = Arc::new(AppState { models });

    let app = Router::new()
        .route("/",get(health))
        .route("/api/tags", get(get_models))
        .route("/api/generate", post(generate))
        .route("/api/chat", post(chat))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:11434")
        .await
        .unwrap();
    println!("🚀 Fake Ollama API server listening on http://localhost:11434");

    axum::serve(listener, app).await.unwrap();
}
