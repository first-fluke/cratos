# 모델 라우팅 전략

## 작업 유형 정의

```rust
pub enum TaskType {
    Classification,    // 분류
    Summarization,     // 요약
    Planning,          // 계획 수립
    CodeGeneration,    // 코드 작성
    ToolCalling,       // 도구 호출
    Polishing,         // 문장 다듬기
}
```

## 라우팅 테이블 (2026 모델)

| TaskType | Anthropic | OpenAI | Gemini | 비용 |
|----------|-----------|--------|--------|------|
| Classification | claude-haiku-4-5 | gpt-5.2-mini | gemini-2.5-flash | 낮음 |
| Summarization | claude-haiku-4-5 | gpt-5.2-mini | gemini-2.5-flash | 낮음 |
| Planning | claude-sonnet-4-5 | gpt-5.2 | gemini-2.5-pro | 중간 |
| CodeGeneration | claude-sonnet-4-5 | gpt-5.2 | gemini-2.5-pro | 중간 |
| ToolCalling | claude-sonnet-4-5 | gpt-5.2 | gemini-2.5-pro | 중간 |
| Polishing | claude-haiku-4-5 | gpt-5.2-mini | gemini-2.5-flash | 낮음 |

## 모델 비용 (2026 기준)

| 모델 | Input $/1M | Output $/1M |
|------|-----------|------------|
| claude-haiku-4-5 | $0.25 | $1.25 |
| claude-sonnet-4-5 | $3.00 | $15.00 |
| claude-opus-4-5 | $15.00 | $75.00 |
| gpt-5.2-mini | $0.15 | $0.60 |
| gpt-5.2 | $2.50 | $10.00 |
| gemini-2.5-flash | $0.075 | $0.30 |
| gemini-2.5-pro | $1.25 | $5.00 |

## 라우터 구현

```rust
pub struct ModelRouter {
    config: RouterConfig,
}

impl ModelRouter {
    pub fn select_model(&self, task: TaskType, context: &Context) -> ModelConfig {
        // 1. 사용자 설정 확인
        if let Some(override_model) = context.user_model_override {
            return override_model;
        }

        // 2. 작업 유형별 기본 모델
        let model = match task {
            TaskType::Classification | TaskType::Summarization | TaskType::Polishing => {
                self.config.fast_model.clone()
            }
            TaskType::Planning | TaskType::CodeGeneration | TaskType::ToolCalling => {
                self.config.smart_model.clone()
            }
        };

        // 3. 비용 제한 확인
        if context.remaining_budget < model.estimated_cost {
            return self.config.fast_model.clone();
        }

        model
    }
}
```

## 비용 추정

```rust
pub fn estimate_cost(model: &str, input_tokens: u32, output_tokens: u32) -> f64 {
    let (input_rate, output_rate) = match model {
        "claude-haiku-4-5" | "gpt-5.2-mini" | "gemini-2.5-flash" => (0.00025, 0.00125),
        "claude-sonnet-4-5" | "gpt-5.2" | "gemini-2.5-pro" => (0.003, 0.015),
        "claude-opus-4-5" => (0.015, 0.075),
        _ => (0.003, 0.015), // 기본값
    };

    let input_cost = (input_tokens as f64 / 1000.0) * input_rate;
    let output_cost = (output_tokens as f64 / 1000.0) * output_rate;

    input_cost + output_cost
}
```

## 폴백 전략

```rust
pub async fn complete_with_fallback(
    &self,
    request: CompletionRequest,
) -> Result<CompletionResponse> {
    // 1. 주 제공자 시도
    match self.primary.complete(&request).await {
        Ok(response) => return Ok(response),
        Err(e) if is_fallback_eligible(&e) => {
            tracing::warn!("Primary provider failed: {}", e);
        }
        Err(e) => return Err(e), // 폴백 불가 에러
    }

    // 2. 폴백 제공자 시도
    self.fallback.complete(&request).await
}

fn is_fallback_eligible(error: &LlmError) -> bool {
    matches!(error,
        LlmError::AuthError(_) |
        LlmError::PermissionDenied(_) |
        LlmError::NetworkError(_) |
        LlmError::Timeout(_) |
        LlmError::RateLimited(_)
    )
}
```

## 프로바이더 우선순위

설정에서 지정:

```toml
[llm]
default_provider = "anthropic"
fallback_providers = ["openai", "gemini", "ollama"]

[llm.routing]
fast_model = "claude-haiku-4-5"
smart_model = "claude-sonnet-4-5"
```
