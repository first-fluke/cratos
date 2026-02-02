# Anthropic 연동 가이드

## 직접 구현 (reqwest)

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
}

impl AnthropicProvider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            api_key: std::env::var("ANTHROPIC_API_KEY").unwrap(),
        }
    }
}
```

## 요청/응답 타입

```rust
#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    id: String,
    content: Vec<ContentBlock>,
    model: String,
    stop_reason: Option<String>,
    usage: Usage,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
    id: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}
```

## API 호출

```rust
impl AnthropicProvider {
    pub async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let anthropic_req = AnthropicRequest {
            model: request.model.unwrap_or("claude-sonnet-4-20250514".into()),
            max_tokens: request.max_tokens.unwrap_or(4096),
            messages: request.messages.into_iter()
                .map(|m| AnthropicMessage {
                    role: m.role,
                    content: m.content,
                })
                .collect(),
            tools: None,
            system: request.system,
        };

        let response = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&anthropic_req)
            .send()
            .await?
            .json::<AnthropicResponse>()
            .await?;

        Ok(CompletionResponse {
            content: response.content[0].text.clone().unwrap_or_default(),
            usage: Some(TokenUsage {
                input: response.usage.input_tokens,
                output: response.usage.output_tokens,
            }),
        })
    }
}
```

## Tool Use

```rust
#[derive(Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

let tools = vec![
    AnthropicTool {
        name: "file_read".into(),
        description: "Read file contents".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            },
            "required": ["path"]
        }),
    },
];
```

## 환경 변수

```bash
ANTHROPIC_API_KEY=sk-ant-your-api-key
```
