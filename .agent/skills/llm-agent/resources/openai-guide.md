# OpenAI 연동 가이드

## async-openai 기본 설정

```rust
use async_openai::{Client, config::OpenAIConfig};

pub struct OpenAIProvider {
    client: Client<OpenAIConfig>,
}

impl OpenAIProvider {
    pub fn new() -> Self {
        let config = OpenAIConfig::new()
            .with_api_key(std::env::var("OPENAI_API_KEY").unwrap());

        Self {
            client: Client::with_config(config),
        }
    }
}
```

## Chat Completion

```rust
use async_openai::types::{
    ChatCompletionRequestMessage,
    CreateChatCompletionRequestArgs,
};

impl OpenAIProvider {
    pub async fn complete(&self, messages: Vec<Message>) -> Result<String> {
        let request = CreateChatCompletionRequestArgs::default()
            .model("gpt-4o")
            .messages(
                messages.into_iter()
                    .map(|m| m.into())
                    .collect::<Vec<ChatCompletionRequestMessage>>()
            )
            .build()?;

        let response = self.client.chat().create(request).await?;

        Ok(response.choices[0].message.content.clone().unwrap_or_default())
    }
}
```

## Tool Calling

```rust
use async_openai::types::{
    ChatCompletionTool,
    ChatCompletionToolType,
    FunctionObject,
};

let tools = vec![
    ChatCompletionTool {
        r#type: ChatCompletionToolType::Function,
        function: FunctionObject {
            name: "file_read".into(),
            description: Some("Read file contents".into()),
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            })),
            strict: Some(true),
        },
    },
];

let request = CreateChatCompletionRequestArgs::default()
    .model("gpt-4o")
    .messages(messages)
    .tools(tools)
    .build()?;
```

## Structured Output

```rust
use async_openai::types::ResponseFormat;

let request = CreateChatCompletionRequestArgs::default()
    .model("gpt-4o")
    .messages(messages)
    .response_format(ResponseFormat::JsonObject)
    .build()?;
```

## 토큰 추적

```rust
if let Some(usage) = response.usage {
    tracing::info!(
        input_tokens = usage.prompt_tokens,
        output_tokens = usage.completion_tokens,
        total_tokens = usage.total_tokens,
        "OpenAI usage"
    );
}
```

## 환경 변수

```bash
OPENAI_API_KEY=sk-your-api-key
OPENAI_ORG_ID=org-your-org-id  # 선택
```
