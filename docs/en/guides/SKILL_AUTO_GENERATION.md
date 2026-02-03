# Skill Auto-generation System

## Overview

Cratos's **Skill Auto-generation System** learns from user tool usage patterns and automatically converts repetitive workflows into reusable skills. This is Cratos's key differentiator.

### Key Features

| Feature | Description |
|---------|-------------|
| **Pattern Learning** | Automatically detects tool sequences repeated 3+ times |
| **High Success Rate** | Generates skills targeting 90%+ success rate |
| **Auto-suggestion** | Prompts users when patterns are detected |
| **Editable** | Generated skills can be modified or disabled |
| **No Docker Required** | Built-in SQLite, runs immediately |

### Competitive Advantages

| Feature | Existing Solutions | Cratos |
|---------|-------------------|--------|
| Skill Creation | Manual marketplace | Auto pattern learning |
| Minimum Learning | N/A | 3 occurrences |
| Keyword Extraction | Manual setup | Automatic extraction |
| Variable Interpolation | Limited | `{{variable}}` syntax |

## Architecture

```
+---------------------------------------------------------------------+
|                          User Input/Actions                          |
+---------------------------------------------------------------------+
                                  |
                                  v
+---------------------------------------------------------------------+
|                    cratos-replay (EventStore)                        |
|  +----------------------------------------------------------------+ |
|  |  - Execution history storage                                    | |
|  |  - Tool call events                                             | |
|  |  - User input text                                              | |
|  +----------------------------------------------------------------+ |
+---------------------------------------------------------------------+
                                  |
                                  v
+---------------------------------------------------------------------+
|                      PatternAnalyzer                                 |
|  +----------------------------------------------------------------+ |
|  |  - Extract tool sequences                                       | |
|  |  - N-gram analysis (2-5 tool combinations)                      | |
|  |  - Keyword extraction (stopword removal)                        | |
|  |  - Confidence score calculation                                 | |
|  +----------------------------------------------------------------+ |
|                                                                     |
|  Configuration:                                                     |
|  - min_occurrences: 3 (minimum occurrence count)                    |
|  - min_confidence: 0.6 (minimum confidence)                         |
|  - max_sequence_length: 5 (maximum sequence length)                 |
|  - analysis_window_days: 30 (analysis period)                       |
+---------------------------------------------------------------------+
                                  |
                                  v
+---------------------------------------------------------------------+
|                      SkillGenerator                                  |
|  +----------------------------------------------------------------+ |
|  |  - Pattern to skill conversion                                  | |
|  |  - Trigger keyword setup                                        | |
|  |  - Execution step generation                                    | |
|  |  - Input schema generation                                      | |
|  +----------------------------------------------------------------+ |
|                                                                     |
|  Generation options:                                                |
|  - min_confidence: 0.7 (skill generation threshold)                 |
|  - auto_activate: false (automatic activation)                      |
|  - max_keywords: 5 (maximum keyword count)                          |
+---------------------------------------------------------------------+
                                  |
                                  v
+---------------------------------------------------------------------+
|                      SkillStore (SQLite)                             |
|  +----------------------------------------------------------------+ |
|  |  Tables:                                                        | |
|  |  - skills: Skill definitions                                    | |
|  |  - detected_patterns: Detected patterns                         | |
|  |  - skill_executions: Execution history                          | |
|  +----------------------------------------------------------------+ |
|                                                                     |
|  Storage location: ~/.cratos/skills.db                              |
+---------------------------------------------------------------------+
                                  |
                                  v
+---------------------------------------------------------------------+
|                    User Request Processing                           |
+---------------------------------------------------------------------+
                                  |
                    +-------------+-------------+
                    v                           v
+-----------------------------+   +-----------------------------------+
|     SkillRouter              |   |   SemanticSkillRouter             |
|  +-------------------------+ |   |   (requires semantic feature)      |
|  |  - Keyword matching     | |   |  +-------------------------------+ |
|  |  - Regex pattern match  | |   |  |  - Vector embedding search    | |
|  |  - Intent classification| |   |  |  - Hybrid matching            | |
|  |  - Priority sorting     | |   |  |  - Semantic similarity        | |
|  +-------------------------+ |   |  +-------------------------------+ |
+-----------------------------+   +-----------------------------------+
                    |                           |
                    +-----------+---------------+
                                v
+---------------------------------------------------------------------+
|                      SkillExecutor                                   |
|  +----------------------------------------------------------------+ |
|  |  - Variable interpolation ({{variable}} -> actual value)        | |
|  |  - Step-by-step execution                                       | |
|  |  - Error handling (Abort/Continue/Retry)                        | |
|  |  - Dry-run mode                                                 | |
|  +----------------------------------------------------------------+ |
|                                                                     |
|  Security settings:                                                 |
|  - max_steps_per_skill: 50                                         |
|  - max_variable_value_length: 100KB                                |
|  - step_timeout_ms: 60000                                          |
+---------------------------------------------------------------------+
                                  |
                                  v
+---------------------------------------------------------------------+
|                    Tool Execution (cratos-tools)                     |
+---------------------------------------------------------------------+
```

## Core Components

### 1. PatternAnalyzer

Detects recurring tool usage patterns from execution history.

```rust
use cratos_skills::{PatternAnalyzer, AnalyzerConfig};

// Create analyzer with default settings
let analyzer = PatternAnalyzer::new();

// Custom configuration
let config = AnalyzerConfig {
    min_occurrences: 3,      // Minimum 3 repetitions
    min_confidence: 0.6,      // 60%+ confidence
    max_sequence_length: 5,   // Max 5-tool sequences
    analysis_window_days: 30, // Analyze last 30 days
};
let analyzer = PatternAnalyzer::with_config(config);

// Detect patterns
let patterns = analyzer.detect_patterns(&event_store).await?;

for pattern in &patterns {
    println!("Pattern: {:?}", pattern.tool_sequence);
    println!("Occurrences: {}", pattern.occurrence_count);
    println!("Confidence: {:.1}%", pattern.confidence_score * 100.0);
    println!("Keywords: {:?}", pattern.extracted_keywords);
}
```

#### Pattern Detection Algorithm

1. **Event Collection**: Query execution history for last N days
2. **Sequence Extraction**: Extract tool call order per execution
3. **N-gram Analysis**: Calculate frequency of 2-5 tool combinations
4. **Confidence Calculation**: `occurrence_count / total_executions`
5. **Keyword Extraction**: Extract from user input after removing stopwords
6. **Pattern Ranking**: Sort by confidence x occurrence count

### 2. SkillGenerator

Converts detected patterns into executable skills.

```rust
use cratos_skills::{SkillGenerator, GeneratorConfig};

let config = GeneratorConfig {
    min_confidence: 0.7,   // Generate only for 70%+
    auto_activate: false,   // Manual activation
    max_keywords: 5,        // Max 5 keywords
};
let generator = SkillGenerator::with_config(config);

// Single pattern to skill
let skill = generator.generate_from_pattern(&pattern)?;
println!("Generated skill: {}", skill.name);
println!("Trigger keywords: {:?}", skill.trigger.keywords);

// Batch conversion
let skills = generator.generate_from_patterns(&patterns);
for (skill, pattern_id) in skills {
    println!("Skill '{}' created (pattern: {})", skill.name, pattern_id);
}
```

#### Generated Skill Structure

```rust
// Example generated skill
Skill {
    name: "file_read_then_git_commit",
    description: "Auto-generated workflow: file_read -> git_commit (triggers: read, commit)",
    category: SkillCategory::Workflow,
    origin: SkillOrigin::AutoGenerated,
    trigger: SkillTrigger {
        keywords: vec!["read", "commit"],
        regex_patterns: vec![],
        intents: vec![],
        priority: 0,
    },
    steps: vec![
        SkillStep {
            order: 1,
            tool_name: "file_read",
            input_template: json!({"path": "{{file_path}}"}),
            on_error: ErrorAction::Abort,
        },
        SkillStep {
            order: 2,
            tool_name: "git_commit",
            input_template: json!({"message": "{{commit_message}}"}),
            on_error: ErrorAction::Continue,
        },
    ],
    input_schema: json!({
        "type": "object",
        "properties": {
            "file_path": {"type": "string"},
            "commit_message": {"type": "string"}
        },
        "required": ["file_path", "commit_message"]
    }),
}
```

### 3. SkillStore

SQLite-based persistent storage.

```rust
use cratos_skills::{SkillStore, default_skill_db_path};

// Use default path (~/.cratos/skills.db)
let store = SkillStore::from_path(&default_skill_db_path()).await?;

// Save skill
store.save_skill(&skill).await?;

// Query skills
let skill = store.get_skill(skill_id).await?;
let skill = store.get_skill_by_name("file_reader").await?;

// List active skills
let active_skills = store.list_active_skills().await?;

// Pattern management
store.save_pattern(&pattern).await?;
store.mark_pattern_converted(pattern_id, skill_id).await?;
store.mark_pattern_rejected(pattern_id).await?;

// Execution tracking
store.record_skill_execution(
    skill_id,
    Some(execution_id),
    true,  // success
    Some(150),  // duration_ms
    &step_results,
).await?;
```

#### Database Schema

```sql
-- Skills table
CREATE TABLE skills (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT 'custom',
    origin TEXT NOT NULL DEFAULT 'user_defined',
    status TEXT NOT NULL DEFAULT 'draft',
    trigger_keywords TEXT NOT NULL DEFAULT '[]',
    trigger_regex_patterns TEXT NOT NULL DEFAULT '[]',
    trigger_intents TEXT NOT NULL DEFAULT '[]',
    trigger_priority INTEGER NOT NULL DEFAULT 0,
    steps TEXT NOT NULL DEFAULT '[]',
    input_schema TEXT,
    usage_count INTEGER NOT NULL DEFAULT 0,
    success_rate REAL NOT NULL DEFAULT 1.0,
    avg_duration_ms INTEGER,
    last_used_at TEXT,
    source_pattern_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Patterns table
CREATE TABLE detected_patterns (
    id TEXT PRIMARY KEY,
    tool_sequence TEXT NOT NULL,
    occurrence_count INTEGER NOT NULL,
    confidence_score REAL NOT NULL,
    extracted_keywords TEXT NOT NULL DEFAULT '[]',
    sample_inputs TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'detected',
    converted_skill_id TEXT,
    detected_at TEXT NOT NULL,
    FOREIGN KEY (converted_skill_id) REFERENCES skills(id)
);

-- Execution tracking table
CREATE TABLE skill_executions (
    id TEXT PRIMARY KEY,
    skill_id TEXT NOT NULL,
    execution_id TEXT,
    success INTEGER NOT NULL,
    duration_ms INTEGER,
    step_results TEXT NOT NULL DEFAULT '[]',
    started_at TEXT NOT NULL,
    FOREIGN KEY (skill_id) REFERENCES skills(id)
);
```

### 4. SkillRouter

Matches user input to appropriate skills.

```rust
use cratos_skills::{SkillRouter, SkillRegistry, RouterConfig};

// Set up registry
let registry = SkillRegistry::new();
let skills = store.list_active_skills().await?;
registry.load_all(skills).await?;

// Router configuration
let config = RouterConfig {
    min_score: 0.3,           // Minimum match score
    keyword_weight: 0.4,       // Keyword weight
    regex_weight: 0.5,         // Regex weight
    intent_weight: 0.6,        // Intent weight
    priority_bonus: 0.1,       // Priority bonus
    max_input_length: 10_000,  // Max input length (DoS prevention)
    max_pattern_length: 500,   // Max pattern length (ReDoS prevention)
};
let mut router = SkillRouter::with_config(registry, config);

// Get all matching skills
let results = router.route("read file and commit").await;
for result in results {
    println!("Skill: {} (score: {:.2})", result.skill.name, result.score);
    println!("Match reason: {:?}", result.match_reason);
}

// Get best match
if let Some(best) = router.route_best("read file and commit").await {
    println!("Selected skill: {}", best.skill.name);
}
```

### 5. SkillExecutor

Executes skills with variable interpolation.

```rust
use cratos_skills::{SkillExecutor, ExecutorConfig, ToolExecutor};
use std::collections::HashMap;

// ToolExecutor implementation required
struct MyToolExecutor { /* ... */ }

#[async_trait]
impl ToolExecutor for MyToolExecutor {
    async fn execute_tool(&self, tool_name: &str, input: Value) -> Result<Value, String> {
        // Tool execution logic
    }
    fn has_tool(&self, tool_name: &str) -> bool { /* ... */ }
    fn tool_names(&self) -> Vec<String> { /* ... */ }
}

// Executor configuration
let config = ExecutorConfig {
    max_retries: 3,
    dry_run: false,
    continue_on_failure: false,
    step_timeout_ms: 60_000,
    max_variable_value_length: 100_000,
    max_steps_per_skill: 50,
};

let executor = SkillExecutor::new(tool_executor)
    .with_config(config);

// Prepare variables
let mut variables = HashMap::new();
variables.insert("file_path".to_string(), json!("/path/to/file.txt"));
variables.insert("commit_message".to_string(), json!("Update file"));

// Execute
let result = executor.execute(&skill, &variables).await?;

if result.success {
    println!("Skill execution successful! ({}ms)", result.total_duration_ms);
    for step in &result.step_results {
        println!("  Step {}: {} - success", step.step, step.tool_name);
    }
} else {
    println!("Skill execution failed: {:?}", result.error);
}
```

## Skill Schema

### Skill Definition

```rust
pub struct Skill {
    pub id: Uuid,                       // Unique ID
    pub name: String,                   // Skill name
    pub description: String,            // Description
    pub category: SkillCategory,        // Workflow | Custom | System
    pub origin: SkillOrigin,            // Builtin | UserDefined | AutoGenerated
    pub status: SkillStatus,            // Draft | Active | Disabled
    pub trigger: SkillTrigger,          // Trigger configuration
    pub steps: Vec<SkillStep>,          // Execution steps
    pub input_schema: Option<Value>,    // JSON Schema
    pub metadata: SkillMetadata,        // Usage statistics
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### SkillStep Definition

```rust
pub struct SkillStep {
    pub order: u32,                    // Execution order (1-based)
    pub tool_name: String,             // Tool name
    pub input_template: Value,         // Input template ({{var}} syntax)
    pub on_error: ErrorAction,         // Abort | Continue | Retry
    pub description: Option<String>,   // Step description
    pub max_retries: u32,              // Maximum retry count
}
```

### SkillTrigger Definition

```rust
pub struct SkillTrigger {
    pub keywords: Vec<String>,         // Trigger keywords
    pub regex_patterns: Vec<String>,   // Regex patterns
    pub intents: Vec<String>,          // Intent classifications
    pub priority: i32,                 // Priority
}
```

## Usage Examples

### Complete Workflow

```rust
use cratos_skills::*;
use cratos_replay::EventStore;

// 1. Initialize stores
let event_store = EventStore::from_path(&default_data_dir()).await?;
let skill_store = SkillStore::from_path(&default_skill_db_path()).await?;

// 2. Analyze patterns
let analyzer = PatternAnalyzer::new();
let patterns = analyzer.detect_patterns(&event_store).await?;

println!("Detected patterns: {}", patterns.len());

// 3. Generate skills
let generator = SkillGenerator::new();
for pattern in &patterns {
    // Skip already converted patterns
    if pattern.status == PatternStatus::Converted {
        continue;
    }

    match generator.generate_from_pattern(pattern) {
        Ok(skill) => {
            // Suggest to user
            println!("\nNew skill discovered!");
            println!("Name: {}", skill.name);
            println!("Description: {}", skill.description);
            println!("Steps: {:?}", skill.steps.iter().map(|s| &s.tool_name).collect::<Vec<_>>());

            // Save and update pattern status
            skill_store.save_skill(&skill).await?;
            skill_store.mark_pattern_converted(pattern.id, skill.id).await?;
        }
        Err(e) => {
            println!("Skill generation failed: {}", e);
        }
    }
}

// 4. Load registry
let registry = SkillRegistry::new();
let active_skills = skill_store.list_active_skills().await?;
registry.load_all(active_skills).await?;

// 5. Route and execute
let mut router = SkillRouter::new(registry);

let user_input = "read file and commit";
if let Some(result) = router.route_best(user_input).await {
    println!("Matched skill: {} (score: {:.2})", result.skill.name, result.score);

    // Execute
    let mut variables = HashMap::new();
    variables.insert("file_path".to_string(), json!("./README.md"));
    variables.insert("commit_message".to_string(), json!("Auto commit"));

    let exec_result = executor.execute(&result.skill, &variables).await?;
    println!("Execution result: {:?}", exec_result.success);
}
```

### Manual Skill Creation

```rust
use cratos_skills::*;

// Define custom skill
let skill = Skill::new(
    "daily_report",
    "Generate and send daily report",
    SkillCategory::Custom,
)
.with_trigger(
    SkillTrigger::with_keywords(vec![
        "daily".to_string(),
        "report".to_string(),
    ])
    .add_pattern(r"daily\s*report")
    .with_priority(10),
)
.with_step(
    SkillStep::new(1, "git_log", json!({
        "since": "{{since_date}}",
        "format": "oneline"
    }))
    .with_description("Get commit log")
    .with_on_error(ErrorAction::Abort),
)
.with_step(
    SkillStep::new(2, "file_write", json!({
        "path": "{{output_path}}",
        "content": "{{step1_output}}"
    }))
    .with_description("Write report file")
    .with_on_error(ErrorAction::Continue),
)
.with_step(
    SkillStep::new(3, "slack_send", json!({
        "channel": "{{channel}}",
        "message": "Daily report has been generated."
    }))
    .with_description("Send Slack notification")
    .with_on_error(ErrorAction::Continue),
);

// Activate and save
skill.activate();
store.save_skill(&skill).await?;
```

## Configuration Options

### AnalyzerConfig

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `min_occurrences` | `u32` | `3` | Minimum occurrences to recognize as pattern |
| `min_confidence` | `f32` | `0.6` | Minimum confidence score |
| `max_sequence_length` | `usize` | `5` | Maximum tool sequence length to analyze |
| `analysis_window_days` | `i64` | `30` | Analysis period in days |

### GeneratorConfig

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `min_confidence` | `f32` | `0.7` | Minimum confidence for skill generation |
| `auto_activate` | `bool` | `false` | Auto-activate on creation |
| `max_keywords` | `usize` | `5` | Maximum keywords in trigger |

### RouterConfig

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `min_score` | `f32` | `0.3` | Minimum score to consider a match |
| `keyword_weight` | `f32` | `0.4` | Keyword matching weight |
| `regex_weight` | `f32` | `0.5` | Regex matching weight |
| `intent_weight` | `f32` | `0.6` | Intent matching weight |
| `priority_bonus` | `f32` | `0.1` | Priority bonus |
| `max_input_length` | `usize` | `10000` | Maximum input length (security) |
| `max_pattern_length` | `usize` | `500` | Maximum pattern length (security) |

### ExecutorConfig

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `max_retries` | `u32` | `3` | Maximum retries per step |
| `dry_run` | `bool` | `false` | Test mode (no actual execution) |
| `continue_on_failure` | `bool` | `false` | Continue on step failure |
| `step_timeout_ms` | `u64` | `60000` | Per-step timeout (ms) |
| `max_variable_value_length` | `usize` | `100000` | Maximum variable value length (security) |
| `max_steps_per_skill` | `usize` | `50` | Maximum steps per skill (security) |

## Security Considerations

### Input Validation

```rust
// Router: Input length limit (DoS prevention)
if input_text.len() > config.max_input_length {
    return Vec::new();  // Reject
}

// Router: Regex length limit (ReDoS prevention)
if pattern.len() > config.max_pattern_length {
    continue;  // Skip
}

// Executor: Step count limit
if skill.steps.len() > config.max_steps_per_skill {
    return Err(Error::Validation("too many steps"));
}

// Executor: Variable value size limit
if value.len() > config.max_variable_value_length {
    return Err(Error::Validation("variable too large"));
}
```

### Recommended Security Settings

1. **Production Environment**
   - `auto_activate: false` - Manual review before activation
   - `max_input_length: 10000` - Input length limit
   - `step_timeout_ms: 30000` - Shorter timeout

2. **Sensitive Environment**
   - Exclude sensitive tools (exec, shell) from skills
   - Whitelist-based tool permissions

## API Reference

### Main Types

| Type | Description |
|------|-------------|
| `PatternAnalyzer` | Pattern analyzer |
| `DetectedPattern` | Detected pattern |
| `PatternStatus` | Detected, Converted, Rejected, Expired |
| `SkillGenerator` | Skill generator |
| `Skill` | Skill definition |
| `SkillCategory` | Workflow, Custom, System |
| `SkillOrigin` | Builtin, UserDefined, AutoGenerated |
| `SkillStatus` | Draft, Active, Disabled |
| `SkillStep` | Execution step |
| `SkillTrigger` | Trigger configuration |
| `ErrorAction` | Abort, Continue, Retry |
| `SkillStore` | SQLite storage |
| `SkillRegistry` | In-memory registry |
| `SkillRouter` | Keyword/regex router |
| `SemanticSkillRouter` | Semantic router (optional) |
| `SkillExecutor` | Skill executor |
| `ToolExecutor` | Tool execution trait |

### Error Types

```rust
pub enum Error {
    SkillNotFound(String),    // Skill not found
    PatternNotFound(String),  // Pattern not found
    Database(String),         // Database error
    Serialization(String),    // Serialization error
    Validation(String),       // Validation error
    Execution(String),        // Execution error
    Configuration(String),    // Configuration error
    Io(std::io::Error),       // IO error
    ReplayStore(cratos_replay::Error),  // Replay store error
    Internal(String),         // Internal error
}
```

## Semantic Router (Optional)

Enable the `semantic` feature to use vector embedding-based semantic search.

```toml
[dependencies]
cratos-skills = { version = "0.1", features = ["semantic"] }
```

```rust
use cratos_skills::{SemanticSkillRouter, SemanticRouterConfig, SkillEmbedder};

// Implement embedding provider
struct MyEmbedder { /* ... */ }

#[async_trait]
impl SkillEmbedder for MyEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> { /* ... */ }
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> { /* ... */ }
    fn dimensions(&self) -> usize { 768 }
}

// Create semantic router
let index = create_skill_index(768, Some(&index_path))?;
let router = SemanticSkillRouter::new(registry, index, embedder);

// Index skills
router.reindex_all().await?;

// Hybrid search (keyword + semantic)
let results = router.route("save the file").await?;  // Can also match "backup" skill
```

## Roadmap

1. **v1.0**: Basic pattern detection and skill generation
2. **v1.1**: Semantic routing (cratos-search integration)
3. **v1.2**: LLM-based intent classification
4. **v2.0**: Skill versioning and rollback
5. **v2.1**: Skill sharing and marketplace
