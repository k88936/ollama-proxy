use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub port: i16,
    pub providers: Vec<ProviderInfo>,
}
#[derive(Serialize, Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub url: String,
    pub secret: Option<String>,
    pub models: Option<Vec<String>>,
    pub api_type: ApiType,
}

#[derive(Serialize, Deserialize)]
pub enum ApiType {
    Ollama,
    Openai,
}

pub fn get_config_demo() -> String {
    let config = Config {
        port: 11434,
        providers: vec![
            ProviderInfo {
                name: "ollama".to_string(),
                url: "https://some.ollama.service:port".to_string(),
                secret: None,
                models: None,
                api_type: ApiType::Ollama,
            },
            ProviderInfo {
                name: "aliyun".to_string(),
                url: "https://dashscope.aliyuncs.com/compatible-mode/".to_string(),
                secret: "secret-key".to_string().into(),
                models: vec![
                    "qwen3-coder-plus",
                    "Moonshot-Kimi-K2-Instruct",
                    "qwen3-max",
                    "glm-4.5",
                ]
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .into(),
                api_type: ApiType::Openai,
            },
            ProviderInfo {
                name: "tsinghua".to_string(),
                url: "https://llmapi.paratera.com".to_string(),
                secret: "secret-key".to_string().into(),
                models: vec!["Qwen3-Coder-Plus", "GLM-4.5"]
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .into(),
                api_type: ApiType::Openai,
            },
        ],
    };
    serde_yaml::to_string(&config).unwrap()
}
