# Graceful Shutdown 가이드

Cratos의 안전한 종료 메커니즘에 대한 가이드입니다.

## 개요

Graceful Shutdown은 시스템이 종료 신호를 받았을 때:
- ❌ 즉시 강제 종료 (데이터 손실 위험)
- ✅ 진행 중인 작업 완료 → 상태 저장 → 연결 정리 → 안전 종료

## 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│                    ShutdownController                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │ CancellToken│  │ Phase State │  │ Active Task Counter │ │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘ │
└─────────┼────────────────┼─────────────────────┼────────────┘
          │                │                     │
          ▼                ▼                     ▼
    ┌──────────┐    ┌──────────┐          ┌──────────┐
    │ HTTP     │    │ Telegram │    ...   │ Agent    │
    │ Server   │    │ Adapter  │          │ Tasks    │
    └──────────┘    └──────────┘          └──────────┘
```

## 종료 단계 (Shutdown Phases)

| Phase | 설명 | 동작 |
|-------|------|------|
| **Running** | 정상 운영 | 모든 요청 처리 |
| **Stopping** | 종료 시작 | 새 요청 거부 |
| **Draining** | 정리 중 | 실행 중 태스크 취소 및 완료 대기 |
| **Terminating** | 강제 종료 | 타임아웃 초과 시 강제 중단 |
| **Terminated** | 종료 완료 | 모든 리소스 해제 |

```
User: Ctrl+C / SIGTERM
           │
           ▼
    ┌──────────────┐
    │   Running    │
    └──────┬───────┘
           │ shutdown() 호출
           ▼
    ┌──────────────┐
    │   Stopping   │  ← 새 작업 거부
    └──────┬───────┘
           │
           ▼
    ┌──────────────┐
    │   Draining   │  ← 태스크 취소, 완료 대기
    └──────┬───────┘
           │
           ├─── 모든 태스크 완료 ───┐
           │                        │
           │ 타임아웃 (30초)        │
           ▼                        │
    ┌──────────────┐                │
    │ Terminating  │  ← 강제 종료   │
    └──────┬───────┘                │
           │                        │
           ▼                        ▼
    ┌──────────────────────────────────┐
    │           Terminated             │
    └──────────────────────────────────┘
```

## 사용법

### 기본 사용

```rust
use cratos_core::{ShutdownController, shutdown_signal_with_controller};

#[tokio::main]
async fn main() {
    // 1. 컨트롤러 생성
    let shutdown = ShutdownController::new();

    // 2. 컴포넌트에 토큰 전달
    let token = shutdown.token();

    tokio::spawn(async move {
        my_component(token).await;
    });

    // 3. 서버 실행 (Ctrl+C 대기)
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal_with_controller(shutdown))
        .await;
}
```

### 컴포넌트에서 취소 처리

```rust
use tokio_util::sync::CancellationToken;

async fn my_component(cancel_token: CancellationToken) {
    loop {
        tokio::select! {
            // 정상 작업
            result = do_work() => {
                handle_result(result);
            }
            // 취소 신호
            _ = cancel_token.cancelled() => {
                info!("종료 신호 수신, 정리 중...");
                cleanup().await;
                return;
            }
        }
    }
}
```

### TaskGuard로 태스크 추적

```rust
async fn process_request(
    shutdown: &ShutdownController,
    request: Request,
) -> Result<Response> {
    // 새 작업 거부 체크
    if !shutdown.is_accepting_work() {
        return Err(Error::ServiceUnavailable);
    }

    // 태스크 등록 (자동 카운트)
    let _guard = shutdown.register_task();

    // 작업 수행
    let result = handle_request(request).await?;

    Ok(result)
    // _guard가 drop되면 자동으로 카운트 감소
}
```

### 수동 태스크 완료 표시

```rust
let guard = shutdown.register_task();

// 작업 수행
do_work().await;

// 명시적 완료 (drop 전에 호출)
guard.complete();
```

## API Reference

### ShutdownController

```rust
impl ShutdownController {
    /// 새 컨트롤러 생성 (기본 타임아웃: 30초)
    pub fn new() -> Arc<Self>;

    /// 커스텀 타임아웃으로 생성
    pub fn with_timeout(timeout: Duration) -> Arc<Self>;

    /// 자식 CancellationToken 발급
    pub fn token(&self) -> CancellationToken;

    /// 종료 phase 변경 구독
    pub fn subscribe(&self) -> broadcast::Receiver<ShutdownPhase>;

    /// 현재 phase 조회
    pub fn phase(&self) -> ShutdownPhase;

    /// 종료 진행 중인지 확인
    pub fn is_shutting_down(&self) -> bool;

    /// 새 작업 수락 가능 여부
    pub fn is_accepting_work(&self) -> bool;

    /// 태스크 등록 (TaskGuard 반환)
    pub fn register_task(&self) -> TaskGuard<'_>;

    /// 활성 태스크 수
    pub fn active_task_count(&self) -> u32;

    /// 그레이스풀 셧다운 시작
    pub async fn shutdown(self: &Arc<Self>);

    /// 즉시 강제 종료
    pub fn force_shutdown(&self);
}
```

### TaskGuard

```rust
impl TaskGuard<'_> {
    /// 태스크 완료 표시
    pub fn complete(self);

    /// 취소 여부 확인
    pub fn is_cancelled(&self) -> bool;

    /// CancellationToken 획득
    pub fn token(&self) -> CancellationToken;
}
// Drop 시 자동으로 active_tasks 감소
```

### Helper Functions

```rust
/// Ctrl+C / SIGTERM 대기
pub async fn wait_for_shutdown_signal();

/// ShutdownController와 통합된 shutdown signal
pub async fn shutdown_signal_with_controller(controller: Arc<ShutdownController>);
```

## 설정

### 타임아웃 설정

```rust
use std::time::Duration;

// 60초 타임아웃
let shutdown = ShutdownController::with_timeout(
    Duration::from_secs(60)
);
```

### Phase 변경 모니터링

```rust
let mut rx = shutdown.subscribe();

tokio::spawn(async move {
    while let Ok(phase) = rx.recv().await {
        match phase {
            ShutdownPhase::Stopping => {
                info!("새 요청 거부 시작");
            }
            ShutdownPhase::Draining => {
                info!("진행 중 태스크 정리 중...");
            }
            ShutdownPhase::Terminated => {
                info!("종료 완료");
                break;
            }
            _ => {}
        }
    }
});
```

## 채널 어댑터 통합

### Telegram 예시

```rust
let telegram_shutdown = shutdown_controller.token();

tokio::spawn(async move {
    tokio::select! {
        result = telegram_adapter.run(orchestrator) => {
            if let Err(e) = result {
                error!("Telegram error: {}", e);
            }
        }
        _ = telegram_shutdown.cancelled() => {
            info!("Telegram adapter shutting down...");
            // 정리 로직
        }
    }
});
```

### Slack 예시

```rust
let slack_shutdown = shutdown_controller.token();

tokio::spawn(async move {
    let mut socket = connect_slack().await?;

    loop {
        tokio::select! {
            msg = socket.recv() => {
                handle_message(msg).await;
            }
            _ = slack_shutdown.cancelled() => {
                socket.close().await;
                info!("Slack adapter closed");
                break;
            }
        }
    }
});
```

## 베스트 프랙티스

### 1. 항상 select! 사용

```rust
// ✅ Good
tokio::select! {
    result = long_running_task() => { /* ... */ }
    _ = cancel_token.cancelled() => { return; }
}

// ❌ Bad - 취소 불가
let result = long_running_task().await;
```

### 2. 정리 로직 구현

```rust
async fn my_service(cancel: CancellationToken) {
    let resource = acquire_resource().await;

    let result = tokio::select! {
        r = do_work(&resource) => r,
        _ = cancel.cancelled() => {
            // 반드시 정리!
            release_resource(resource).await;
            return;
        }
    };

    release_resource(resource).await;
}
```

### 3. 새 작업 거부

```rust
async fn handle_request(shutdown: &ShutdownController) -> Result<()> {
    if !shutdown.is_accepting_work() {
        return Err(Error::ServiceUnavailable);
    }

    let _guard = shutdown.register_task();
    // ...
}
```

### 4. 타임아웃 적절히 설정

```rust
// 짧은 작업 서비스
let shutdown = ShutdownController::with_timeout(Duration::from_secs(10));

// 긴 작업 서비스 (AI 처리 등)
let shutdown = ShutdownController::with_timeout(Duration::from_secs(60));
```

## 트러블슈팅

### 종료가 안 됨

1. **원인**: 태스크가 `cancel_token.cancelled()` 체크 안 함
2. **해결**: 모든 장기 실행 태스크에 `tokio::select!` 추가

### 타임아웃 발생

1. **원인**: 태스크 정리가 타임아웃보다 오래 걸림
2. **해결**:
   - 타임아웃 늘리기
   - 정리 로직 최적화
   - 강제 종료 허용

### 로그 확인

```
INFO  Initiating graceful shutdown...
INFO  Shutdown phase changed: Stopping
INFO  Shutdown phase changed: Draining
DEBUG Waiting for tasks to complete... active_tasks=3 elapsed_secs=0
DEBUG Waiting for tasks to complete... active_tasks=1 elapsed_secs=5
INFO  All tasks completed gracefully
INFO  Shutdown phase changed: Terminated
INFO  Graceful shutdown complete
```

## 관련 문서

- [CancellationToken](./CANCELLATION_TOKEN.md)
- [Token Budget](./TOKEN_BUDGET.md)
- [Agent Orchestrator](./ORCHESTRATOR.md)
