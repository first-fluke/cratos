# Rust 에러 플레이북

## 컴파일 에러

### E0382: borrow of moved value

```rust
// 문제
let s = String::from("hello");
let t = s;
println!("{}", s); // Error!

// 해결 1: Clone
let t = s.clone();

// 해결 2: 참조 사용
let t = &s;
```

### E0597: borrowed value does not live long enough

```rust
// 문제
fn bad() -> &str {
    let s = String::from("hello");
    &s // Error: s는 함수 종료 시 drop
}

// 해결: 소유권 반환
fn good() -> String {
    String::from("hello")
}
```

### E0277: trait bound not satisfied

```rust
// 문제
async fn send<T>(val: T) {
    tokio::spawn(async move { val }); // Error: T: Send 필요
}

// 해결: trait bound 추가
async fn send<T: Send + 'static>(val: T) {
    tokio::spawn(async move { val });
}
```

## 런타임 에러

### tokio: "Cannot start a runtime from within a runtime"

```rust
// 문제
#[tokio::main]
async fn main() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {}); // Error!
}

// 해결: 이미 런타임 안에 있으므로 직접 await
#[tokio::main]
async fn main() {
    some_async_fn().await;
}
```

### sqlx: "no database URL found"

```rust
// 문제: DATABASE_URL 환경변수 없음

// 해결 1: .env 파일
DATABASE_URL=postgres://user:pass@localhost/db

// 해결 2: 런타임 설정
let pool = PgPoolOptions::new()
    .connect(&std::env::var("DATABASE_URL")?)
    .await?;
```

## 라이프타임 에러

### 'static lifetime required

```rust
// 문제
fn spawn_task(s: &str) {
    tokio::spawn(async move {
        println!("{}", s); // Error: 'static 필요
    });
}

// 해결: 소유권 전달
fn spawn_task(s: String) {
    tokio::spawn(async move {
        println!("{}", s);
    });
}
```

## 디버깅 팁

1. `RUST_BACKTRACE=1` 환경변수로 백트레이스 활성화
2. `cargo expand`로 매크로 확장 확인
3. `cargo tree`로 의존성 충돌 확인
4. `#[derive(Debug)]` 추가 후 `dbg!()` 매크로 사용
