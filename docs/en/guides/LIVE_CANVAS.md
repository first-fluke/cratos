# Live Canvas - Real-time Visual Workspace

## Overview

Live Canvas is an interactive space where AI agents visualize work results in real-time and collaborate with users.

### Core Features

| Feature | Description |
|---------|-------------|
| **Real-time Streaming** | Display LLM responses chunk by chunk instantly |
| **Multimodal Rendering** | Integrated display of code, diagrams, images, charts |
| **Bidirectional Editing** | Users can directly modify canvas content |
| **Version Control** | Track change history and revert |
| **Collaboration Support** | Multi-cursor, simultaneous editing |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Live Canvas UI                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ Code Panel  │  │  Diagram    │  │   Preview Panel     │  │
│  │ (Monaco)    │  │  (Mermaid)  │  │   (Rendered HTML)   │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                           │
                   WebSocket (Real-time)
                           │
┌─────────────────────────────────────────────────────────────┐
│                    Canvas Server (Rust)                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ Session Mgr │  │  Document   │  │   Renderer Engine   │  │
│  │ (tokio)     │  │  Store      │  │   (markdown/code)   │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                           │
┌─────────────────────────────────────────────────────────────┐
│                    Cratos Core                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ Orchestrator│  │    LLM      │  │      Replay         │  │
│  │             │  │  (Streaming)│  │    (History)        │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Tech Stack

### Backend (Rust)

```toml
# crates/cratos-canvas/Cargo.toml
[dependencies]
axum = { version = "0.7", features = ["ws"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }

# Real-time communication
tokio-tungstenite = "0.21"
futures = "0.3"

# Document processing
pulldown-cmark = "0.10"     # Markdown parsing
syntect = "5"               # Code highlighting
```

### Frontend

```json
{
  "dependencies": {
    "react": "^18.2.0",
    "@monaco-editor/react": "^4.6.0",
    "mermaid": "^10.6.0",
    "katex": "^0.16.0",
    "y-websocket": "^1.5.0",
    "yjs": "^13.6.0"
  }
}
```

## Core Types

```rust
/// Canvas session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasSession {
    pub id: Uuid,
    pub user_id: String,
    pub document: CanvasDocument,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Canvas document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasDocument {
    pub blocks: Vec<CanvasBlock>,
    pub version: u64,
}

/// Canvas block types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CanvasBlock {
    /// Markdown text
    Markdown {
        id: Uuid,
        content: String,
    },
    /// Code block
    Code {
        id: Uuid,
        language: String,
        content: String,
        executable: bool,
    },
    /// Mermaid diagram
    Diagram {
        id: Uuid,
        diagram_type: DiagramType,
        source: String,
    },
    /// Image
    Image {
        id: Uuid,
        url: String,
        alt: String,
    },
    /// Chart (Chart.js/D3)
    Chart {
        id: Uuid,
        chart_type: ChartType,
        data: serde_json::Value,
    },
}

/// Diagram types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiagramType {
    Flowchart,
    Sequence,
    ClassDiagram,
    StateDiagram,
    EntityRelationship,
    Gantt,
}
```

## WebSocket Protocol

### Client → Server

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// Join session
    Join { session_id: Uuid },

    /// Update block
    UpdateBlock {
        block_id: Uuid,
        content: String,
        cursor_position: Option<usize>,
    },

    /// Add block
    AddBlock {
        after_id: Option<Uuid>,
        block: CanvasBlock,
    },

    /// Delete block
    DeleteBlock { block_id: Uuid },

    /// Move block
    MoveBlock {
        block_id: Uuid,
        new_position: usize,
    },

    /// Request to AI
    AskAI {
        prompt: String,
        context_blocks: Vec<Uuid>,
    },
}
```

### Server → Client

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    /// Session state
    SessionState { document: CanvasDocument },

    /// Block updated (by another user or AI)
    BlockUpdated {
        block_id: Uuid,
        content: String,
        source: UpdateSource,
    },

    /// AI streaming response
    AIStreaming {
        block_id: Uuid,
        chunk: String,
        is_complete: bool,
    },

    /// Error
    Error { message: String },

    /// Cursor position (collaboration)
    CursorUpdate {
        user_id: String,
        block_id: Uuid,
        position: usize,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum UpdateSource {
    User(String),
    AI,
    System,
}
```

## Usage Examples

### 1. Code Generation and Execution

```
[User] "Create a Fibonacci function in Rust"

[AI adds code block to canvas]
┌─────────────────────────────────────────┐
│ ```rust                                  │
│ fn fibonacci(n: u64) -> u64 {           │
│     match n {                            │
│         0 => 0,                          │
│         1 => 1,                          │
│         _ => fibonacci(n-1) + fib...    │ ← Real-time streaming
│     }                                    │
│ }                                        │
│ ```                                      │
│ [▶ Run] [📋 Copy]                        │
└─────────────────────────────────────────┘
```

### 2. Diagram Generation

```
[User] "Draw this system architecture as a diagram"

[AI adds Mermaid diagram to canvas]
┌─────────────────────────────────────────┐
│ ┌─────────┐     ┌─────────┐             │
│ │  User   │────▶│   API   │             │
│ └─────────┘     └────┬────┘             │
│                      │                   │
│               ┌──────┴──────┐           │
│               ▼             ▼           │
│          ┌─────────┐  ┌─────────┐       │
│          │   LLM   │  │   DB    │       │
│          └─────────┘  └─────────┘       │
│ [🔧 Edit] [📥 Download PNG]              │
└─────────────────────────────────────────┘
```

### 3. Collaborative Editing

```
[User A editing]
┌─────────────────────────────────────────┐
│ # Project Plan                           │
│                                          │
│ ## Phase 1: Design|                      │ ← User A cursor
│ - Requirements analysis  [🟢 User B]     │ ← User B cursor
│ - Architecture design                    │
│                                          │
│ [🤖 AI adding Phase 3...]                │
└─────────────────────────────────────────┘
```

## Replay Integration

Live Canvas integrates with Cratos's Replay system to track all changes:

```rust
/// Canvas event (for Replay storage)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasEvent {
    pub session_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub event_type: CanvasEventType,
    pub actor: ActorType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CanvasEventType {
    BlockAdded(CanvasBlock),
    BlockUpdated { block_id: Uuid, old: String, new: String },
    BlockDeleted(Uuid),
    BlockMoved { block_id: Uuid, from: usize, to: usize },
    AIRequestStarted { prompt: String },
    AIResponseCompleted { block_id: Uuid },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActorType {
    User(String),
    AI(String),  // Model name
}
```

### Timeline View

```
[Canvas Timeline]
┌────────────────────────────────────────────────────────────┐
│ 10:00:00 │ 👤 User   │ Created new document                 │
│ 10:00:15 │ 👤 User   │ Requested "Create fibonacci function"│
│ 10:00:16 │ 🤖 AI     │ Started adding code block            │
│ 10:00:18 │ 🤖 AI     │ Code block completed (45 tokens)     │
│ 10:00:25 │ 👤 User   │ Modified code: "u64" → "u128"        │
│ 10:00:30 │ 👤 User   │ Clicked [▶ Run]                      │
├──────────┴───────────┴───────────────────────────────────────┤
│ [◀ Revert] [Re-execute ▶] [dry-run 🔍]                       │
└────────────────────────────────────────────────────────────┘
```

## Configuration

```toml
# config/default.toml
[canvas]
enabled = true
port = 8081
max_sessions = 100
max_blocks_per_document = 500

# WebSocket settings
[canvas.websocket]
heartbeat_interval_secs = 30
max_message_size_kb = 1024

# Collaboration settings
[canvas.collaboration]
enabled = true
max_users_per_session = 10
sync_interval_ms = 100

# AI streaming settings
[canvas.ai_streaming]
enabled = true
chunk_size = 20  # Token unit
```

## API Endpoints

```
# REST API
GET  /api/canvas/sessions              # List sessions
POST /api/canvas/sessions              # Create new session
GET  /api/canvas/sessions/:id          # Get session
DELETE /api/canvas/sessions/:id        # Delete session

# WebSocket
WS   /api/canvas/ws/:session_id        # Real-time connection

# Export
GET  /api/canvas/sessions/:id/export   # Export Markdown/HTML
POST /api/canvas/sessions/:id/snapshot # Save snapshot
```

## Security

- Session-specific authentication tokens required
- JWT verification on WebSocket connection
- Per-block permission management (read/write)
- Rate limiting (requests per minute)

## Roadmap

1. **v1.0**: Basic canvas + code/markdown
2. **v1.1**: Diagram support (Mermaid)
3. **v1.2**: Collaboration features (Yjs)
4. **v2.0**: Code execution (WebAssembly sandbox)
