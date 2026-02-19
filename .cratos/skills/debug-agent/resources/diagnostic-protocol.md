# 진단 프로토콜

## 4단계 진단 프로세스

### 1단계: 증상 수집

```
수집 항목:
- 에러 메시지 전문
- 스택 트레이스
- 실행 컨텍스트 (환경, 입력값)
- 최근 변경 사항
```

### 2단계: 원인 분류

```rust
pub enum ErrorCategory {
    // 환경 문제
    Environment {
        missing_env: Vec<String>,
        wrong_version: Option<String>,
    },

    // 권한 문제
    Permission {
        resource: String,
        required: String,
        current: String,
    },

    // 네트워크 문제
    Network {
        host: String,
        port: u16,
        error: String,
    },

    // 코드 버그
    CodeBug {
        file: String,
        line: u32,
        description: String,
    },

    // 의존성 문제
    Dependency {
        package: String,
        version_conflict: Option<String>,
    },
}
```

### 3단계: 진단 실행

```rust
pub struct Diagnostic {
    pub category: ErrorCategory,
    pub checks: Vec<DiagnosticCheck>,
}

pub struct DiagnosticCheck {
    pub name: String,
    pub command: String,
    pub expected: String,
    pub actual: Option<String>,
    pub passed: bool,
}

// 예시: 환경 변수 체크
DiagnosticCheck {
    name: "OPENAI_API_KEY 확인".into(),
    command: "echo $OPENAI_API_KEY | head -c 10".into(),
    expected: "sk-...".into(),
    actual: Some("(not set)".into()),
    passed: false,
}
```

### 4단계: 해결 가이드 생성

```markdown
## 진단 결과: API 키 누락

### 문제
OpenAI API 키가 설정되지 않았습니다.

### 해결 방법

1. **환경 변수 설정**
   ```bash
   export OPENAI_API_KEY=sk-your-key-here
   ```

2. **.env 파일 사용**
   ```bash
   echo "OPENAI_API_KEY=sk-your-key" >> .env
   ```

3. **설정 파일 확인**
   ```bash
   cat config/default.toml | grep api_key
   ```

### 검증
```bash
curl https://api.openai.com/v1/models \
  -H "Authorization: Bearer $OPENAI_API_KEY"
```
```

## 일반적인 진단 명령어

```bash
# 환경 변수 확인
env | grep -E "(API_KEY|TOKEN|SECRET)"

# 네트워크 연결 테스트
curl -I https://api.openai.com
nc -zv localhost 5432

# Rust 의존성 충돌
cargo tree -d

# 파일 권한 확인
ls -la /path/to/file

# 프로세스 확인
lsof -i :19527
```
