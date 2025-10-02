use crate::models::{Message, Model, StreamChatChunk};
use crate::providers::{ChatChunkStream, Provider, ProviderError};
use chrono;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;
#[derive(Clone)]
pub struct OllamaProvider {
    base_url: String,
    password: String,
    models: Vec<Model>,
}

#[derive(Deserialize)]
struct OllamaChatChunk {
    message: Option<MessageContent>,
    done: bool,
}

#[derive(Deserialize)]
struct MessageContent {
    content: String,
}

impl OllamaProvider {
    pub fn new( base_url: String, password: String, models: Vec<Model>) -> Self {
        Self {
            base_url,
            password,
            models
        }
    }

    fn build_client(&self) -> Result<reqwest::Client, ProviderError> {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| ProviderError {
                message: format!("Failed to build HTTP client: {}", e),
            })
    }
}
fn build_request_body(model: &String, messages: &[Message], option: Option<Value>) -> Value {
    let msgs: Vec<Value> = messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();

    // Build base body
    let mut body = json!({
        "model": model,
        "messages": msgs,
        "stream": true,
    });

    // Merge options if provided
    if let Some(Value::Object(opts)) = option {
        if let Some(obj) = body.as_object_mut() {
            for (k, v) in opts {
                obj.insert(k, v);
            }
        }
    }

    body
}

#[async_trait::async_trait]
impl Provider for OllamaProvider {
    fn chat(
        &self,
        model: &String,
        messages: &[Message],
        option: Option<Value>,
    ) -> Result<ChatChunkStream, ProviderError> {
        let client = self.build_client()?;

        let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));

        let body = build_request_body(model, messages, option);

        let model_name = model.clone();

        let password = self.password.clone();
        let stream = async_stream::stream! {
            let request_builder = client
                .post(&url)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", password))
                .json(&body);

            let response = match request_builder
                .send()
                .await
            {
                Ok(response) => response,
                Err(e) => {
                    yield Err(ProviderError {
                        message: format!("HTTP request failed: {}", e),
                    });
                    return;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                yield Err(ProviderError {
                    message: format!("HTTP error {}: {}", status, error_text),
                });
                return;
            }

            let mut stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk_result) = stream.next().await {
                let chunk = match chunk_result {
                    Ok(chunk) => chunk,
                    Err(e) => {
                        yield Err(ProviderError {
                            message: format!("Stream read error: {}", e),
                        });
                        return;
                    }
                };

                let chunk_str = match std::str::from_utf8(&chunk) {
                    Ok(s) => s,
                    Err(e) => {
                        yield Err(ProviderError {
                            message: format!("UTF-8 decode error: {}", e),
                        });
                        return;
                    }
                };

                buffer.push_str(chunk_str);

                // Process complete lines
                while let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].trim().to_string();
                    buffer.drain(..=line_end);

                    if line.is_empty() {
                        continue;
                    }

                    match serde_json::from_str::<OllamaChatChunk>(&line) {
                        Ok(chunk) => {
                            if let Some(message) = chunk.message {
                                // Add delay between chunks to simulate realistic streaming
                                tokio::time::sleep(Duration::from_millis(20)).await;
                                let thunk = StreamChatChunk {
                                    model: model_name.clone(),
                                    created_at: chrono::Utc::now().to_rfc3339(),
                                    message: Message {
                                        role: "assistant".to_string(),
                                        content: message.content.clone(),
                                    },
                                    done: chunk.done,
                                };

                                yield Ok(thunk);

                                if chunk.done {
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            yield Err(ProviderError {
                                message: format!("JSON parse error: {}", e),
                            });
                            return;
                        }
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }


    async fn get_models(&self) -> Vec<Model> {
        self.models.clone()
    }
}
