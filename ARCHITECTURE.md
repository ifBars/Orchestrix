# Architecture

System design and technical architecture of Orchestrix.

## Table of Contents

- [High-Level Architecture](#high-level-architecture)
- [Data Flow](#data-flow)
- [UX and Performance Guardrails](#ux-and-performance-guardrails)
- [Component Overview](#component-overview)
- [Event System](#event-system)
- [Database Schema](#database-schema)
- [IPC Contract](#ipc-contract)
- [State Management](#state-management)

## High-Level Architecture

Orchestrix follows a **backend-authoritative, event-driven** architecture where the Rust backend owns all state and orchestration, while the React frontend renders state and handles user input.

```
┌─────────────────────────────────────────────────────────────────────┐
│                           Frontend                                  │
│                      (React + TypeScript)                           │
├──────────────┬──────────────┬──────────────┬──────────────────────┤
│   Sidebar    │     Chat     │  Artifacts   │      Settings        │
│              │   Timeline   │    Panel     │                      │
├──────────────┴──────────────┴──────────────┴──────────────────────┤
│                         Zustand Stores                              │
│                    (appStore, streamStore)                          │
└─────────────────────────────────┬───────────────────────────────────┘
                                  │ invoke / events
                                  │ (Tauri IPC)
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         Backend (Rust)                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │   Commands   │  │ Orchestrator │  │   Planner    │              │
│  │  (Tauri)     │  │              │  │  (MiniMax/   │              │
│  │              │  │ - Task Mgmt  │  │   Kimi)      │              │
│  │ - create_    │  │ - Run Coord  │  │              │              │
│  │   task       │  │ - Sub-agent  │  │ - Multi-turn │              │
│  │ - run_plan_  │  │   delegation │  │   plan loop  │              │
│  │   mode       │  │ - Approval   │  │ - Tools +    │              │
│  │ - run_build_ │  │   gates      │  │   create_    │              │
│  │ - list_      │  │              │  │   artifact   │              │
│  │   tasks      │  │              │  │              │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │    Tools     │  │   Event Bus  │  │  Worktrees   │              │
│  │              │  │              │  │              │              │
│  │ - Filesystem │  │ - Publish    │  │ - Git branch │              │
│  │ - Commands   │  │ - Subscribe  │  │   isolation  │              │
│  │ - Search     │  │ - Batching   │  │ - Conflict   │              │
│  │ - Git        │  │ - Persist    │  │   detection  │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │   Policy     │  │   Recovery   │  │    Skills    │              │
│  │   Engine     │  │              │  │              │              │
│  │              │  │ - Crash      │  │ - Registry   │              │
│  │ - Path       │  │   recovery   │  │ - MCP proto  │              │
│  │   sandboxing │  │ - Resume     │  │ - Dynamic    │              │
│  │ - Command    │  │   runs       │  │   loading    │              │
│  │   allowlist  │  │              │  │              │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
└─────────────────────────────────┬───────────────────────────────────┘
                                  │
                                  ▼
                    ┌──────────────────────────┐
                    │        SQLite            │
                    │  ┌────────┐ ┌─────────┐  │
                    │  │ tasks  │ │  runs   │  │
                    │  └────────┘ └─────────┘  │
                    │  ┌────────┐ ┌─────────┐  │
                    │  │ events │ │tool_    │  │
                    │  └────────┘ │  calls  │  │
                    │  ┌────────┐ └─────────┘  │
                    │  │sub_    │ ┌─────────┐  │
                    │  │ agents │ │artifacts│  │
                    │  └────────┘ └─────────┘  │
                    └──────────────────────────┘
```

## Data Flow

### Task Lifecycle

```
User Input
    │
    ▼
┌─────────────────┐
│  create_task()  │  Create task record in DB
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  start_task()   │  Invokes run_plan_mode (planning phase)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Multi-turn      │  Plan-mode agent loop: decide_worker_action
│ plan loop       │  → tool calls (fs.list, fs.read, …) → execute
│ (planner)       │  → until agent.create_artifact → extract plan
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ artifact.created│  Plan artifact written; agent.plan_ready
│ plan_ready      │  emitted. UI displays plan for review
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ approve_plan()  │  User approves plan
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Orchestrator.   │  Execute plan steps
│   execute()     │
└────────┬────────┘
         │
         ├────────────────┬────────────────┐
         │                │                │
         ▼                ▼                ▼
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│  Tool Call  │  │  Sub-agent  │  │   LLM Call  │
│             │  │ delegation  │  │             │
└─────────────┘  └─────────────┘  └─────────────┘
         │                │                │
         ▼                ▼                ▼
┌─────────────────────────────────────────────────┐
│              Events Emitted                     │
│  - tool.call_started/finished                   │
│  - agent.subagent_started/completed             │
│  - agent.step_started/completed                 │
└─────────────────────────────────────────────────┘
```

### Event Flow

```
Backend (Rust)
    │
    │ 1. Event generated
    ▼
┌─────────────────┐
│   EventBus.     │  Publish to broadcast channel
│     emit()      │
└────────┬────────┘
         │
         │ 2. Event broadcast
         ├──────────────────────────────────────►
         │                                        │
         │                              ┌─────────────┐
         │                              │  EventBatcher │
         │                              │  (100ms batch) │
         │                              └──────┬──────┘
         │                                     │
         │ 3. Batch to frontend                │
         │◄────────────────────────────────────┘
         │
         ▼
┌─────────────────┐
│   Tauri Event   │  orchestrix://events
│    Channel      │
└────────┬────────┘
         │
         │ 4. WebSocket-like stream
         ▼
Frontend (React)
    │
    ▼
┌─────────────────┐
│    listen()     │  Subscribe to events
│   (Tauri API)   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  eventBuffer.   │  Transform to conversation items
│    process()    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Zustand Store  │  Update UI state
│   (appStore)    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   React Re-     │  Re-render components
│    render       │
└─────────────────┘
```

## UX and Performance Guardrails

Orchestrix UX is designed to keep users continuously informed without flooding them with low-value noise.

### Human-in-the-Loop Checkpoints

- **Plan gate**: execution starts only after explicit user plan approval
- **Execution oversight**: user can monitor, cancel, and provide feedback while runs are active
- **Policy gate**: sensitive operations are permission-gated and auditable
- **Artifact review**: outcomes stay inspectable with run/step/tool traceability

### Transparency Requirements

- Every meaningful model/tool transition emits an event
- No hidden background actions that bypass the timeline
- Frontend state must be reconstructible from events + database records
- Timeline items should preserve task/run/step correlation IDs

### Condensed Visualization Patterns

- Use step/phase grouping to collapse repetitive event bursts
- Show compact summaries first, expand to raw args/output on demand
- Prioritize failures/warnings in the visual hierarchy
- Keep one authoritative execution timeline per run

### Performance and Scale Constraints

- Backend emits append-only events and batches high-frequency traffic
- Critical feedback events flush immediately for responsiveness
- Frontend uses selector-based subscriptions and incremental event transforms
- Active message streaming uses dedicated stream state so token deltas do not rebuild the full timeline list
- Long timelines should be windowed/virtualized to keep rendering responsive

## Component Overview

### Frontend Components

| Component | File | Purpose |
|-----------|------|---------|
| `IdeShell` | `layouts/IdeShell.tsx` | Main app layout shell |
| `Sidebar` | `components/Sidebar.tsx` | Task list navigation |
| `ChatInterface` | `components/Chat/ChatInterface.tsx` | Main chat area |
| `ConversationTimeline` | `components/Chat/ConversationTimeline.tsx` | Renders event stream as chat |
| `Composer` | `components/Composer.tsx` | User input area |
| `ArtifactPanel` | `components/Artifacts/ArtifactPanel.tsx` | File/artifact review |
| `SettingsSheet` | `components/Settings/SettingsSheet.tsx` | Configuration UI |

### Frontend Stores

| Store | File | Purpose |
|-------|------|---------|
| `appStore` | `stores/appStore.ts` | Main application state (tasks, events) |
| `streamStore` | `stores/streamStore.ts` | High-frequency reactive counters |
| `eventBuffer` | `runtime/eventBuffer.ts` | Event-to-conversation mapper |

### Backend Modules

| Module | Path | Purpose |
|--------|------|---------|
| `commands` | `commands/` | Tauri command handlers (IPC entry points) |
| `orchestrator` | `runtime/orchestrator/` | Task execution and coordination |
| `planner` | `runtime/planner.rs` | Multi-turn plan generation (tool loop + agent.create_artifact) |
| `event_bus` | `bus/event_bus.rs` | Event publishing and subscription |
| `batcher` | `bus/batcher.rs` | Event batching for frontend |
| `tools` | `tools/` | Tool registry and implementations |
| `policy` | `policy/` | Permission and sandboxing |
| `db` | `db/` | Database layer and migrations |
| `model` | `model/` | LLM API clients |

## Event System

### Event Structure

```rust
pub struct BusEvent {
    pub id: String,           // UUID v4
    pub run_id: Option<String>,  // Associated run
    pub seq: i64,             // Monotonic sequence
    pub category: String,     // Namespace
    pub event_type: String,   // Specific event
    pub payload: serde_json::Value,
    pub created_at: String,   // RFC 3339
}
```

### Event Categories

| Category | Events | Description |
|----------|--------|-------------|
| `task` | `task.created`, `task.status_changed` | Task lifecycle |
| `agent` | `agent.planning_started`, `agent.deciding`, `agent.tool_calls_preparing`, `agent.plan_ready`, `agent.plan_message`, `agent.plan_delta`, `agent.message_stream_started`, `agent.message_delta`, `agent.message_stream_completed`, `agent.message_stream_cancelled`, `agent.step_started`, `agent.subagent_started`, `agent.subagent_completed` | Agent execution |
| `tool` | `tool.call_started`, `tool.call_finished` | Tool invocations |
| `artifact` | `artifact.created` | Generated artifacts |
| `system` | `system.error`, `system.warning` | System events |

### Event Batching

High-frequency events are batched before reaching the frontend:
- **Flush interval**: 100ms
- **Max batch size**: 50 events
- **Immediate flush set**: `task.*`, `agent.step_*`, `agent.deciding`, `agent.tool_calls_preparing`
- **Delivery guarantee**: At-least-once (persisted to DB)

## Database Schema

### Core Tables

```sql
-- Tasks (top-level work items)
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,           -- UUID
    prompt TEXT NOT NULL,          -- User request
    parent_task_id TEXT,           -- For sub-tasks
    status TEXT NOT NULL,          -- pending/planning/executing/completed/failed
    created_at TEXT NOT NULL,      -- RFC 3339
    updated_at TEXT NOT NULL
);

-- Runs (execution instances)
CREATE TABLE runs (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    status TEXT NOT NULL,
    plan_json TEXT,                -- Serialized plan
    started_at TEXT NOT NULL,
    finished_at TEXT,
    failure_reason TEXT
);

-- Sub-agents (parallel execution units)
CREATE TABLE sub_agents (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    step_idx INTEGER,
    name TEXT NOT NULL,
    status TEXT NOT NULL,
    worktree_path TEXT,
    context_json TEXT,
    started_at TEXT,
    finished_at TEXT,
    error TEXT
);

-- Tool calls (audit log)
CREATE TABLE tool_calls (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    step_idx INTEGER,
    tool_name TEXT NOT NULL,
    input_json TEXT NOT NULL,
    output_json TEXT,
    status TEXT NOT NULL,          -- started/completed/failed
    started_at TEXT,
    finished_at TEXT,
    error TEXT
);

-- Events (event sourcing)
CREATE TABLE events (
    id TEXT PRIMARY KEY,
    run_id TEXT,
    seq INTEGER NOT NULL,
    category TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- Artifacts (generated files)
CREATE TABLE artifacts (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    uri_or_content TEXT NOT NULL,
    metadata_json TEXT,
    created_at TEXT NOT NULL
);
```

### Key Indexes

```sql
CREATE INDEX idx_events_run_id ON events(run_id);
CREATE INDEX idx_events_seq ON events(seq);
CREATE INDEX idx_tool_calls_run_id ON tool_calls(run_id);
CREATE INDEX idx_sub_agents_run_id ON sub_agents(run_id);
```

## IPC Contract

### Commands (Frontend → Backend)

All commands use Tauri's `invoke()`:

```typescript
// Task management
const task = await invoke<TaskRow>("create_task", { prompt, options });
await invoke("start_task", { taskId });
const tasks = await invoke<TaskRow[]>("list_tasks");

// Plan operations
await invoke("approve_plan", { runId });
await invoke("submit_plan_feedback", { runId, feedback });

// Provider config
await invoke("set_provider_config", { provider, config });
const configs = await invoke<ProviderConfigView[]>("get_provider_configs");
```

### Events (Backend → Frontend)

Events are received via Tauri's `listen()`:

```typescript
import { listen } from "@tauri-apps/api/event";

const unlisten = await listen<BusEvent[]>("orchestrix://events", (event) => {
    // Process batched events
    for (const evt of event.payload) {
        handleEvent(evt);
    }
});
```

### Type Mirroring

TypeScript interfaces must exactly mirror Rust structs:

```rust
// Rust
#[derive(Serialize)]
pub struct TaskRow {
    pub id: String,
    pub prompt: String,
    pub status: String,
}
```

```typescript
// TypeScript
interface TaskRow {
    id: string;
    prompt: string;
    status: TaskStatus;
}
```

## State Management

### Frontend State

**appStore** (`stores/appStore.ts`):
- Task list and selected task
- Conversation items (transformed events)
- Settings and configuration
- Actions: createTask, startTask, deleteTask, etc.

**streamStore** (`stores/streamStore.ts`):
- High-frequency counters (plan ticks, event counts)
- Used for reactive UI updates without re-rendering entire tree

### Backend State

**AppState** (`lib.rs`):
```rust
pub struct AppState {
    pub db: Arc<Database>,         // SQLite connection
    pub bus: Arc<EventBus>,        // Event broadcasting
    pub orchestrator: Arc<Orchestrator>,  // Task execution
}
```

All shared state uses `Arc` for thread-safe reference counting.

### State Flow

1. User action → Frontend store action
2. Store calls `invoke()` command
3. Backend updates database
4. Backend emits events via EventBus
5. EventBatcher batches and sends to frontend
6. Frontend receives events, applies incremental transforms, updates stores
7. React re-renders based on store changes

## Security Model

### Sandboxing

- **Path sandboxing**: Tools can only access workspace directory
- **Command allowlist**: Only approved commands can execute
- **Policy engine**: All tool calls evaluated before execution

### Approval Gates

Certain operations require explicit user approval:
- Command execution outside sandbox
- File writes to protected paths
- Git operations on protected branches

## Extension Points

### Adding Tools

1. Implement `Tool` trait in `tools/`
2. Register in tool registry
3. Add to policy allowlist if needed

### Adding Skills

Skills are MCP-compatible:
1. Create skill manifest in `.agents/skills/`
2. Implement skill logic
3. Register via Settings UI

### Adding Commands

1. Add handler function in `commands/`
2. Register in `lib.rs` `generate_handler!` macro
3. Add TypeScript types in `types/index.ts`

---

## References

### Documentation
- [AGENTS.md](./AGENTS.md) - Agent architecture and execution model
- [DESIGN_SYSTEM.md](./DESIGN_SYSTEM.md) - Visual design tokens and UI standards
- [UX_PRINCIPLES.md](./UX_PRINCIPLES.md) - UX, transparency, and performance guardrails
- [SETUP.md](./SETUP.md) - Development environment setup
- [CODING_STANDARDS.md](./CODING_STANDARDS.md) - Code conventions and standards

### Skills
- **orchestrix-app-development** - Use when implementing Orchestrix features (see `.agents/skills/orchestrix-app-development/SKILL.md`)

---

For implementation details, see [CODING_STANDARDS.md](./CODING_STANDARDS.md) and [UX_PRINCIPLES.md](./UX_PRINCIPLES.md).
