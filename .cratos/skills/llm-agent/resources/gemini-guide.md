# Gemini 연동 가이드

## 지원 모델

| 모델 | 용도 | 비고 |
|------|------|------|
| gemini-2.5-pro | 고급 추론, 코드 | thinking 지원 |
| gemini-2.5-flash | 빠른 응답, 분류 | 저비용 |
| gemini-2.5-flash-lite | 초경량 | 최저비용 |

## 인증 방식

### 1. Standard API (권장)

```bash
# 환경 변수
GEMINI_API_KEY=your_api_key
```

### 2. OAuth (AI Pro 구독)

```bash
# gcloud CLI로 인증
gcloud auth application-default login --scopes='https://www.googleapis.com/auth/cloud-platform,https://www.googleapis.com/auth/generative-language'
```

Cratos 설정:

```toml
[llm.gemini]
auth_method = "oauth"  # 또는 "api_key"
```

## API 호출 예시

```rust
use reqwest::Client;
use serde_json::json;

async fn call_gemini(prompt: &str) -> Result<String> {
    let client = Client::new();
    let api_key = std::env::var("GEMINI_API_KEY")?;

    let response = client
        .post(format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={}",
            api_key
        ))
        .json(&json!({
            "contents": [{
                "parts": [{"text": prompt}]
            }]
        }))
        .send()
        .await?;

    // 응답 파싱
    let result: serde_json::Value = response.json().await?;
    Ok(result["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string())
}
```

## Function Calling

```rust
let request = json!({
    "contents": [{"parts": [{"text": prompt}]}],
    "tools": [{
        "function_declarations": [{
            "name": "search_web",
            "description": "Search the web",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                },
                "required": ["query"]
            }
        }]
    }]
});
```

## 주의사항

### 스키마 필드 제한

Gemini API는 일부 JSON Schema 필드를 지원하지 않습니다:

```rust
// 제거해야 할 필드
const UNSUPPORTED_FIELDS: &[&str] = &[
    "default",
    "additionalProperties",
];

fn strip_unsupported_schema_fields(schema: &mut serde_json::Value) {
    if let Some(obj) = schema.as_object_mut() {
        for field in UNSUPPORTED_FIELDS {
            obj.remove(*field);
        }
        for (_, v) in obj.iter_mut() {
            strip_unsupported_schema_fields(v);
        }
    }
}
```

### thought_signature 보존 (Gemini 3)

Gemini 3 모델은 function call에 `thoughtSignature`를 반환합니다. 다음 요청에서 반드시 보존해야 합니다:

```rust
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
    pub thought_signature: Option<String>,  // 보존 필수!
}
```

### Code Assist API 사용 금지

> **경고**: `cloudcode-pa.googleapis.com` (Code Assist API)는 제3자 사용 시 계정 영구 밴 위험이 있습니다.
> Standard API (`generativelanguage.googleapis.com`)만 사용하세요.

## 할당량 (Tier 1, 빌링 활성화)

| 모델 | RPD | TPM |
|------|-----|-----|
| Flash | 2,000 | 250K |
| Pro | 100 | 32K |

## 폴백 전략

```rust
async fn complete_with_fallback(&self, request: CompletionRequest) -> Result<Response> {
    match self.complete_internal(&request).await {
        Ok(r) => Ok(r),
        Err(e) if is_fallback_eligible(&e) => {
            // Auth/Permission, Network, Timeout 에러 시 폴백
            self.fallback_provider.complete(&request).await
        }
        Err(e) => Err(e),
    }
}

fn is_fallback_eligible(error: &LlmError) -> bool {
    matches!(error,
        LlmError::AuthError(_) |
        LlmError::PermissionDenied(_) |
        LlmError::NetworkError(_) |
        LlmError::Timeout(_)
    )
}
```
