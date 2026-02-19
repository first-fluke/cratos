---
name: Model Routing
description: This skill should be used when implementing LLM model routing for cost optimization - Cratos key differentiator.
version: 1.0.0
---

# Model Routing

Cratos 차별화 기능 - 작업별 모델 자동 선택으로 비용 70% 절감.

## 작업 유형

```rust
pub enum TaskType {
    Classification,    // 분류 → Haiku
    Summarization,     // 요약 → Haiku
    Planning,          // 계획 → Sonnet
    CodeGeneration,    // 코드 → Sonnet/Opus
    ToolCalling,       // 도구 → Sonnet
    Polishing,         // 다듬기 → Haiku
}
```

## 라우팅 테이블

| TaskType | Anthropic | OpenAI | 비용/1M |
|----------|-----------|--------|---------|
| Classification | claude-3-haiku | gpt-4o-mini | $0.25 |
| Summarization | claude-3-haiku | gpt-4o-mini | $0.25 |
| Planning | claude-sonnet-4 | gpt-4o | $3.00 |
| CodeGeneration | claude-sonnet-4 | gpt-4o | $3.00 |
| Polishing | claude-3-haiku | gpt-4o-mini | $0.25 |

## 라우터 구현

```rust
pub struct ModelRouter {
    config: RouterConfig,
}

impl ModelRouter {
    pub fn select_model(&self, task: TaskType, context: &Context) -> ModelConfig {
        // 1. 사용자 오버라이드 확인
        if let Some(m) = context.user_model_override {
            return m;
        }

        // 2. 작업 유형별 선택
        match task {
            TaskType::Classification |
            TaskType::Summarization |
            TaskType::Polishing => self.config.fast_model.clone(),

            TaskType::Planning |
            TaskType::CodeGeneration |
            TaskType::ToolCalling => self.config.smart_model.clone(),
        }
    }
}
```

## 비용 추정

```rust
pub fn estimate_cost(model: &str, input: u32, output: u32) -> f64 {
    let (in_rate, out_rate) = match model {
        "claude-3-haiku" | "gpt-4o-mini" => (0.00025, 0.00125),
        "claude-sonnet-4" | "gpt-4o" => (0.003, 0.015),
        _ => (0.003, 0.015),
    };
    (input as f64 / 1000.0) * in_rate + (output as f64 / 1000.0) * out_rate
}
```

## 참조

- `.cratos/skills/llm-agent/resources/routing-strategy.md`
