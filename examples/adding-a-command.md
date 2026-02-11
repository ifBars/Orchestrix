# Adding a New Tauri Command

This guide shows how to add a new command that can be called from the frontend.

## Overview

Tauri commands are Rust functions exposed to the frontend via IPC. They are the primary way the frontend interacts with the backend, and they should support transparent, human-in-the-loop UX through auditable event emission.

## Step-by-Step Guide

### 1. Create the Command Function

Create a new file in `src-tauri/src/commands/` or add to an existing module:

```rust
// src-tauri/src/commands/my_feature.rs

use tauri::State;
use crate::{AppState, AppError};
use crate::db::queries;

/// Gets statistics for a specific task.
/// 
/// # Arguments
/// * `state` - Application state (automatically injected by Tauri)
/// * `task_id` - UUID of the task to analyze
/// 
/// # Returns
/// Task statistics including duration, tool call count, etc.
#[tauri::command]
pub async fn get_task_stats(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<TaskStats, AppError> {
    // Access the database from state
    let db = &state.db;
    
    // Fetch data from database
    let task = queries::get_task(db, &task_id)?;
    let runs = queries::list_runs_for_task(db, &task_id)?;
    
    // Calculate statistics
    let stats = TaskStats {
        task_id: task.id,
        total_runs: runs.len() as i64,
        total_duration_ms: calculate_duration(&runs),
        status: task.status,
    };
    
    Ok(stats)
}

// Define the return type
#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskStats {
    pub task_id: String,
    pub total_runs: i64,
    pub total_duration_ms: i64,
    pub status: String,
}
```

### 2. Export the Command

Add the command to the module's public exports:

```rust
// src-tauri/src/commands/mod.rs

pub mod tasks;
pub mod runs;
pub mod my_feature;  // Add this line

// Re-export all commands for easy importing
pub use tasks::*;
pub use runs::*;
pub use my_feature::*;  // Add this line
```

### 3. Register the Command

Add the command to the `generate_handler!` macro in `lib.rs`:

```rust
// src-tauri/src/lib.rs

.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    
    // Add your new command here
    commands::my_feature::get_task_stats,
])
```

### 4. Add TypeScript Types

Create TypeScript types that mirror the Rust structs:

```typescript
// src/types/index.ts

// Add to existing types
export interface TaskStats {
  task_id: string;
  total_runs: number;
  total_duration_ms: number;
  status: string;
}
```

### 5. Call from Frontend

Use the command in your React components:

```tsx
// src/components/TaskStats/TaskStats.tsx

import { invoke } from "@tauri-apps/api/core";
import type { TaskStats } from "@/types";

export function TaskStats({ taskId }: { taskId: string }) {
  const [stats, setStats] = useState<TaskStats | null>(null);
  const [loading, setLoading] = useState(true);
  
  useEffect(() => {
    const loadStats = async () => {
      try {
        const data = await invoke<TaskStats>("get_task_stats", { taskId });
        setStats(data);
      } catch (error) {
        console.error("Failed to load stats:", error);
      } finally {
        setLoading(false);
      }
    };
    
    loadStats();
  }, [taskId]);
  
  if (loading) return <div>Loading...</div>;
  if (!stats) return <div>No data</div>;
  
  return (
    <div>
      <h3>Task Statistics</h3>
      <p>Runs: {stats.total_runs}</p>
      <p>Duration: {stats.total_duration_ms}ms</p>
      <p>Status: {stats.status}</p>
    </div>
  );
}
```

## Command Patterns

### With State Access

Most commands need access to application state:

```rust
#[tauri::command]
pub async fn my_command(
    state: State<'_, AppState>,
    // ... other params
) -> Result<..., AppError> {
    let db = &state.db;
    let bus = &state.bus;
    let orchestrator = &state.orchestrator;
    
    // Use state components...
}
```

### Async Commands

Use `async` for I/O operations:

```rust
#[tauri::command]
pub async fn fetch_data(
    state: State<'_, AppState>,
) -> Result<Data, AppError> {
    // This runs in an async context
    let result = tokio::time::timeout(
        Duration::from_secs(30),
        perform_async_operation()
    ).await?;
    
    Ok(result)
}
```

### Error Handling

Always return `Result<T, AppError>`:

```rust
#[tauri::command]
pub fn my_command() -> Result<String, AppError> {
    // Propagate errors with ?
    let data = fetch_data().map_err(|e| {
        AppError::Other(format!("Failed to fetch: {}", e))
    })?;
    
    // Or use ? with From trait
    let parsed = parse_data(&data)?;  // Automatically converts to AppError
    
    Ok(parsed)
}
```

### Parameter Types

Use owned types for parameters (Tauri serializes them):

```rust
// Good
#[tauri::command]
pub fn command(name: String, count: i32, options: Option<MyOptions>) { }

// Avoid - references don't work
#[tauri::command]
pub fn bad_command(name: &str) { }
```

## Testing Commands

Add tests in `src-tauri/src/testing.rs`:

```rust
#[test]
fn test_get_task_stats() {
    let db = test_db();
    
    // Create test task
    let task = create_test_task(&db);
    
    // Create test runs
    create_test_run(&db, &task.id);
    
    // Call command (synchronously for testing)
    let rt = tokio::runtime::Runtime::new().unwrap();
    let state = create_test_state(db);
    
    let stats = rt.block_on(async {
        get_task_stats(State::from(&state), task.id).await
    }).unwrap();
    
    assert_eq!(stats.total_runs, 1);
}
```

## Best Practices

1. **Document your command** - Add doc comments explaining what it does
2. **Use meaningful parameter names** - Prefer `task_id` over `id`
3. **Validate inputs** - Check parameters before processing
4. **Return structured types** - Don't return raw strings or tuples
5. **Handle errors gracefully** - Return `AppError` for failures
6. **Keep commands focused** - One command should do one thing
7. **Add TypeScript types** - Mirror Rust types exactly
8. **Emit auditable events** - Surface command progress/outcomes to the timeline
9. **Design for summary + detail** - Return concise status fields plus inspectable metadata

## Common Issues

### Command not found

Make sure you added the command to `generate_handler!` in `lib.rs`.

### Type mismatch

Ensure TypeScript types exactly match Rust struct field names (snake_case).

### State not available

Verify `AppState` is properly managed in `lib.rs`:
```rust
.manage(state)
```

## See Also

- [Tauri Commands Documentation](https://tauri.app/v1/guides/features/command/)
- [ARCHITECTURE.md](../ARCHITECTURE.md) - IPC Contract section
- [CODING_STANDARDS.md](../CODING_STANDARDS.md) - Rust Standards
