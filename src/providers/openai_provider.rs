use crate::models::{Message, Model, StreamChatChunk};
use crate::providers::{ChatChunkStream, Provider, ProviderError};
use chrono;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;
#[derive(Clone)]
pub struct OpenAIProvider {
    key: String,
    models: Vec<Model>,
    base_url: String,
}

#[derive(Deserialize)]
struct OpenaiChatChunk {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    delta: Option<Delta>,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
}

impl OpenAIProvider {
    pub fn new(base_url: String, key: String, models: Vec<Model>) -> Self {
        Self {
            key,
            base_url,
            models,
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

    fn build_request_body(
        &self,
        model: &String,
        messages: &[Message],
        option: Option<Value>,
    ) -> Value {
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
        if let Some(Value::Object(opts)) = option
            && let Some(obj) = body.as_object_mut()
        {
            for (k, v) in opts {
                obj.insert(k, v);
            }
        }

        body
    }

    fn build_request(
        &self,
        model: &String,
        messages: &[Message],
        option: Option<Value>,
    ) -> Result<reqwest::RequestBuilder, ProviderError> {
        let client = self.build_client()?;
        let url = format!(
            "{}/chat/completions",
            self.base_url.trim_end_matches('/')
        );
        let body = self.build_request_body(model, messages, option);
        let key = self.key.clone();

        let builder = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", key))
            .header("Content-Type", "application/json")
            .json(&body);

        Ok(builder)
    }
}

#[async_trait::async_trait]
impl Provider for OpenAIProvider {
    fn chat(
        &self,
        model: &String,
        messages: &[Message],
        option: Option<Value>,
    ) -> Result<ChatChunkStream, ProviderError> {
        let model_name = model.clone();
        let request = self.build_request(model, messages, option)?;

        let stream = async_stream::stream! {
            let response = match request
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
            let mut stream_ended = false;

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

                    if line == "data: [DONE]" {
                        stream_ended = true;
                        break;
                    }

                    if let Some(data) = line.strip_prefix("data: ") {
                        match serde_json::from_str::<OpenaiChatChunk>(data) {
                            Ok(chunk) => {
                                if let Some(choice) = chunk.choices.first()
                                    && let Some(delta) = &choice.delta
                                    && let Some(content) = &delta.content
                                {
                                    let thunk = StreamChatChunk {
                                        model: model_name.clone(),
                                        created_at: chrono::Utc::now().to_rfc3339(),
                                        message: Message {
                                            role: "assistant".to_string(),
                                            content: content.clone(),
                                        },
                                        done: false,
                                    };

                                    yield Ok(thunk);
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

                if stream_ended {
                    break;
                }
            }

            // Send a final "done" message
            let final_chunk = StreamChatChunk {
                model: model_name,
                created_at: chrono::Utc::now().to_rfc3339(),
                message: Message {
                    role: "assistant".to_string(),
                    content: "".to_string(),
                },
                done: true,
            };
            yield Ok(final_chunk);
        };

        Ok(Box::pin(stream))
    }

    async fn get_models(&self) -> Vec<Model> {
        self.models.clone()
    }
}
