# Coding Standards

This document outlines the coding standards and best practices for the Orchestrix project. Following these guidelines ensures consistency, maintainability, and quality across the codebase. The current codebase is a proof-of-concept and may not yet conform to all of these standards — this document defines the target state.

## Table of Contents

- [General Principles](#general-principles)
- [TypeScript / React Standards](#typescript--react-standards)
- [Rust Standards](#rust-standards)
- [Styling Standards](#styling-standards)
- [UX Standards](#ux-standards)
- [State Management](#state-management)
- [Event System](#event-system)
- [Database Conventions](#database-conventions)
- [IPC Contract](#ipc-contract)
- [File Organization](#file-organization)
- [Naming Conventions](#naming-conventions)
- [Testing Standards](#testing-standards)
- [Linting and Formatting](#linting-and-formatting)
- [Comments and Documentation](#comments-and-documentation)

## General Principles

### Code Quality

- **Write self-documenting code**: Prefer clear variable and function names over excessive comments
- **Keep it simple**: Avoid over-engineering; implement what's needed, not what might be needed
- **Single Responsibility**: Each function, component, or module should have one clear purpose
- **DRY (Don't Repeat Yourself)**: Extract common logic into reusable utilities or components
- **Type Safety First**: Leverage TypeScript's type system fully; avoid `any` types. Use Rust's type system to enforce invariants at compile time

### Performance

- **Lazy Loading**: Use React lazy imports for large components that aren't needed on initial render
- **Memoization**: Use `useMemo` and `useCallback` appropriately to prevent unnecessary re-renders
- **Bundle Size**: Keep dependencies minimal; prefer lighter alternatives when possible
- **Event Batching**: High-frequency events are batched before reaching the frontend (100ms flush, 50-item max). UI code should never assume single-event delivery
- **Incremental Processing**: Process incoming events incrementally; avoid full timeline recomputation on every batch
- **Long List Strategy**: Use virtualization/windowing for long timelines or task lists

### Architecture

- **Backend-authoritative**: All orchestration, state, and execution live in the Rust backend
- **Event-driven UI**: The frontend renders state via streamed events; it never controls logic
- **Plan-first execution**: Every task begins with a structured planning phase
- **Human-in-the-loop by default**: Plan review and intervention points are first-class
- **Transparency-first UX**: Users can inspect model/tool activity throughout runs
- **Condensed visualization**: Prioritize high-signal summaries with expandable detail
- **Minimal surface area**: No embedded editor, no live code manipulation by humans

### UX and Transparency

- **No hidden transitions**: Any meaningful agent or tool transition must produce an auditable event
- **Progressive disclosure**: Show concise summary rows first; expand to full raw details on demand
- **Single source of truth**: Timeline status and badges derive from event stream + DB, not duplicated local heuristics
- **Readable at scale**: Group repetitive events by phase/step and elevate errors/warnings in hierarchy
- **Human control always available**: Cancel/review actions stay visible while runs are active

## TypeScript / React Standards

### Component Structure

Use functional components with TypeScript types for props:

```tsx
type ComposerProps = {
    onSend: (prompt: string) => void;
    isDisabled: boolean;
};

export function Composer({ onSend, isDisabled }: ComposerProps) {
    // Component logic
}
```

### Component Organization

Order component elements consistently:

1. Imports
2. Type definitions
3. Constants
4. Main component function
5. Sub-components (if small, single-use, and not reusable)
6. Exports (if not inline)

### Named Exports

Prefer named `export function` over default exports for all components:

```tsx
// Good
export function Sidebar({ onOpenSettings }: SidebarProps) {
    // ...
}

// Avoid
export default function Sidebar(props: SidebarProps) {
    // ...
}
```

The sole exception is the root `App` component.

### Sub-Components

File-private sub-components are defined as non-exported functions in the same file. Small, single-use sub-components define their props inline:

```tsx
// ConversationTimeline.tsx
export function ConversationTimeline(props: ConversationTimelineProps) { ... }

function UserMessage({ prompt, relatedTasks }: { prompt: string; relatedTasks: TaskRow[] }) { ... }
function ToolCallItem({ item }: { item: ConversationItem }) { ... }
```

### TypeScript Configuration

- **Strict Mode**: Always enabled (`strict: true`)
- **No Unused Variables**: Enforced via `noUnusedLocals` and `noUnusedParameters`
- **No Fallthrough**: Enforced via `noFallthroughCasesInSwitch`
- **Path Aliases**: Use `@/*` for imports from `src/`

```tsx
// Good
import { useAppStore } from "@/store";
import { Button } from "@/components/ui/button";

// Avoid
import { useAppStore } from "../../store";
```

### Hooks Usage

- Place hooks at the top of the component, before any conditional logic
- Use custom hooks to encapsulate complex logic
- Custom hooks live in a `hooks/` subdirectory within their feature folder
- Prefer Zustand selectors over prop drilling for global state

```tsx
export function ChatInterface() {
    const [tasks, selectedTaskId, selectTask] = useAppStore(
        useShallow((state) => [state.tasks, state.selectedTaskId, state.selectTask])
    );
    const [localState, setLocalState] = useState(false);

    useEffect(() => {
        // Effect logic
    }, [dependency]);

    // Rest of component
}
```

### Zustand Selectors

Always use `useShallow` from `zustand/shallow` when selecting multiple values to prevent unnecessary re-renders:

```tsx
// Good - multiple values with useShallow
const [tasks, selectedTaskId] = useAppStore(
    useShallow((state) => [state.tasks, state.selectedTaskId])
);

// Good - single scalar selector (no useShallow needed)
const selectedTaskId = useAppStore((state) => state.selectedTaskId);

// Avoid - selecting the entire store
const store = useAppStore();
```

### Event Handlers

- Prefix event handlers with `handle`: `handleClick`, `handleSubmit`, `handleSend`
- Define handlers inside the component unless they need to be extracted for reuse

### Props Destructuring

Always destructure props in the component signature for clarity:

```tsx
// Good
function Button({ className, variant, size, ...props }: ButtonProps) {
    // ...
}

// Avoid
function Button(props: ButtonProps) {
    const { className, variant, size } = props;
    // ...
}
```

### Conditional Rendering

```tsx
// Short conditions
{isAvailable && <Component />}

// Binary choice
{isActive ? <ActiveIcon /> : <InactiveIcon />}

// Complex conditions - extract to variable
const shouldRenderPanel = isPlanReady && hasSteps;
{shouldRenderPanel && <PlanPanel />}
```

### Async Error Handling

Store actions that are `async` use try/catch internally. Fire-and-forget async calls in event handlers use `.catch(console.error)`:

```tsx
onClick={() => submit().catch(console.error)}
onClick={() => deleteTask(task.id).catch(console.error)}
```

Prefer `async/await` over promise chains everywhere else:

```tsx
// Good
try {
    const data = await invoke<TaskRow[]>("list_tasks");
    processData(data);
} catch (error) {
    console.error("Failed to fetch tasks:", error);
}

// Avoid
invoke("list_tasks")
    .then(data => processData(data))
    .catch(error => console.error(error));
```

### Class Usage

Classes are used sparingly, only for stateful singletons (e.g., `RuntimeEventBuffer`). Prefer plain functions and Zustand stores for all other cases.

## Rust Standards

### Module Organization

- Organize related functionality into modules under descriptive directories
- Use `mod.rs` for directory re-exports
- Keep Tauri command handlers in `lib.rs` (or a dedicated `commands/` module as the app grows)
- Keep domain logic in its own module (`runtime/`, `model/`, `tools/`, etc.)

```rust
// bus/mod.rs
mod batcher;
mod event_bus;

pub use batcher::EventBatcher;
pub use event_bus::{BusEvent, EventBus};
```

Private submodules use `mod` without `pub`. Shared internal helpers use `pub(super)` visibility.

### Error Handling

Every module defines its own `thiserror` error enum, named `{Domain}Error`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("migration failed: {0}")]
    Migration(String),
    #[error("not found: {0}")]
    NotFound(String),
}
```

Error message style: lowercase first word, no trailing period.

The top-level `AppError` wraps domain errors and implements `Serialize` as a plain string for Tauri command returns:

```rust
impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
```

**Rules**:
- Never use `.unwrap()` in production code. Use `.expect("reason")` only in tests or provably-safe contexts
- Provide meaningful error messages: `.map_err(|e| format!("failed to read config: {}", e))?`
- Propagate errors with `?` wherever possible

### Tauri Commands

```rust
#[tauri::command]
async fn create_task(
    state: tauri::State<'_, AppState>,
    prompt: String,
    options: Option<CreateTaskOptions>,
) -> Result<TaskRow, AppError> {
    // ...
}
```

Conventions:
- First parameter is always `state: tauri::State<'_, AppState>`
- Return `Result<T, AppError>` for fallible commands
- Use `async` when the command does I/O or awaits
- Parameters use owned types (`String`, not `&str`) per Tauri's serde requirements
- Optional parameters use `Option<T>`

### Struct Derive Patterns

```rust
// Data transfer objects (sent to frontend)
#[derive(Debug, Clone, Serialize)]

// Data received from frontend
#[derive(Debug, Clone, Deserialize)]

// Both directions
#[derive(Debug, Clone, Serialize, Deserialize)]

// Enums with string serialization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]

// Tagged enums (discriminated unions for TypeScript)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
```

### State Management

All shared state is wrapped in `Arc`. Interior mutability uses `std::sync::Mutex` for synchronous data and `tokio::sync::broadcast` for async channels:

```rust
struct AppState {
    db: Arc<Database>,
    bus: Arc<EventBus>,
    orchestrator: Arc<Orchestrator>,
}
```

### Trait Definitions

Async trait methods use `#[allow(async_fn_in_trait)]` rather than the `async-trait` crate:

```rust
#[allow(async_fn_in_trait)]
pub trait PlannerModel: Send + Sync {
    fn model_id(&self) -> &'static str;
    async fn generate_plan(&self, req: PlannerRequest) -> Result<Plan, ModelError>;
}
```

### Import Order

```rust
// 1. Standard library
use std::path::{Path, PathBuf};
use std::sync::Arc;

// 2. External crates
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// 3. Internal crate modules
use crate::bus::EventBus;
use crate::core::plan::{Plan, PlanStep, StepStatus};
use crate::db::{queries, Database};
```

### Constants

Constants use `SCREAMING_SNAKE_CASE` and are defined at module scope:

```rust
const BUS_CAPACITY: usize = 1024;
const DEFAULT_FLUSH_INTERVAL: Duration = Duration::from_millis(100);
const WORKER_MAX_TURNS: usize = 8;
```

### UUID and Timestamps

- UUIDs: `uuid::Uuid::new_v4().to_string()` for all identifiers
- Timestamps: `chrono::Utc::now().to_rfc3339()` for all datetime fields
- Both stored as `TEXT` in SQLite

### Tracing

Use `tracing` macros for structured logging:
- `tracing::info!` for operational milestones (migrations applied, task started)
- `tracing::warn!` for recoverable issues (event bus publish failures, lagged events)
- `tracing::error!` for failures that affect correctness

### Feature Flags

Use feature flags for optional functionality:

```rust
#[cfg(feature = "profiling")]
mod profiling;
```

## Styling Standards

### Tailwind CSS v4

The project uses Tailwind CSS v4 with the Vite plugin (`@tailwindcss/vite`). Configuration lives in `src/index.css`, not in a standalone config file.

### Color System (OKLCH)

All colors use the OKLCH color space, defined as CSS custom properties in `:root` and `.dark`:

```css
:root {
    --primary: oklch(0.58 0.16 245);
    --primary-foreground: oklch(0.985 0 0);
}

.dark {
    --primary: oklch(0.68 0.14 235);
    --primary-foreground: oklch(0.14 0.01 260);
}
```

Dark mode uses a class-based strategy (`document.documentElement.classList.toggle("dark")`).

### Color Token Naming

Follows the shadcn/ui pattern with `{role}` and `{role}-foreground` pairs:

| Token | Purpose |
|-------|---------|
| `background` / `foreground` | Page base |
| `card` / `card-foreground` | Card surfaces |
| `primary` / `primary-foreground` | Primary actions |
| `secondary` / `secondary-foreground` | Secondary actions |
| `muted` / `muted-foreground` | Subdued surfaces/text |
| `accent` / `accent-foreground` | Hover/focus highlights |
| `destructive` / `destructive-foreground` | Danger actions |
| `border` | Default border color |
| `ring` | Focus ring color |
| `success`, `warning`, `info` | Semantic status colors |
| `sidebar-*` | Sidebar-specific tokens |

### Theme Mapping

CSS variables are mapped to Tailwind via `@theme inline`:

```css
@theme inline {
    --color-background: var(--background);
    --color-primary: var(--primary);
    --radius-sm: calc(var(--radius) - 4px);
}
```

### Class Variance Authority (CVA)

Use CVA for component variants in `src/components/ui/`:

```tsx
const buttonVariants = cva("base-classes...", {
    variants: {
        variant: {
            default: "...",
            secondary: "...",
            ghost: "...",
            outline: "...",
            destructive: "...",
        },
        size: {
            default: "h-9 px-4 py-2",
            sm: "h-8 px-3",
            lg: "h-10 px-5",
            icon: "size-9",
        },
    },
    defaultVariants: {
        variant: "default",
        size: "default",
    },
});
```

### Tailwind Merge

Always use the `cn()` utility from `@/lib/utils` to merge classes:

```tsx
import { cn } from "@/lib/utils";

<button className={cn(buttonVariants({ variant, size }), className)} />
```

### Data Slot Pattern

UI primitives use `data-slot` attributes for targeting from parent styles:

```tsx
<button data-slot="button" />
<input data-slot="input" />
```

### Icons

All icons come from `lucide-react`. Icon size is set via the `size` prop, not className:

```tsx
import { Plus, Settings, Trash2 } from "lucide-react";

<Plus size={14} />
<Settings size={14} />
```

Standard sizes: `10`, `12`, `13`, `14`, `16`, `24`.

## UX Standards

### Agent Timeline UX

- **Keep users involved**: planning, execution, and completion states should always be visible and actionable
- **Phase-first view**: clearly present `planning`, `awaiting_review`, `executing`, `completed`, and `failed`
- **Summary-first rendering**: default to concise timeline rows (status, one-line summary, timestamp)
- **Expandable diagnostics**: raw tool args/output/logs must be available without leaving context
- **Error prominence**: warnings and failures should appear before routine informational entries

### Human-in-the-Loop Review UX

- Plan approval UI must be explicit and blocking before build-mode execution
- Feedback loops should preserve context (task, run, step references)
- Permission-gated actions should display why approval is required
- Completion views should link artifacts back to execution steps and tool calls

### Non-Cluttered Visualization Rules

- Avoid duplicate status surfaces that disagree with timeline truth
- Group bursty events under step/phase containers
- Collapse repetitive success items when detail density is high
- Keep copy short, concrete, and stateful (what happened, where, result)

## State Management

### Zustand Stores

The primary store lives in `src/store.ts` as a single `useAppStore`. Secondary stores (e.g., `streamStore.ts`) handle high-frequency reactive counters.

```typescript
type AppStoreState = {
    // State
    tasks: TaskRow[];
    selectedTaskId: string | null;
    // Actions
    bootstrap: () => Promise<void>;
    shutdown: () => void;
    createTask: (prompt: string, options?: CreateTaskOptions) => Promise<void>;
};

export const useAppStore = create<AppStoreState>((set, get) => ({
    // Implementation
}));
```

### Store Rules

- Use TypeScript types for state shape
- Provide individual action functions, not a generic `setState`
- Export derived selector hooks from the store file
- Keep store actions `async` when they call backend commands
- Handle errors inside store actions with try/catch

### Selector Hooks

Export derived selectors as standalone hooks:

```typescript
export const useTaskPlanTick = (taskId: string | null) =>
    useStreamTickStore((state) => (taskId ? state.planTickByTask[taskId] ?? 0 : 0));
```

## Event System

### Event Structure

```rust
pub struct BusEvent {
    pub id: String,           // UUID
    pub run_id: Option<String>,
    pub seq: i64,             // Monotonic sequence
    pub category: String,     // Namespace
    pub event_type: String,   // Specific event
    pub payload: serde_json::Value,
    pub created_at: String,   // RFC 3339
}
```

### Event Naming

Events use `{category}.{action}` dotted notation:

| Category | Event Types |
|----------|-------------|
| `task` | `task.status_changed` |
| `agent` | `agent.planning_started`, `agent.deciding`, `agent.tool_calls_preparing`, `agent.plan_ready`, `agent.plan_message`, `agent.plan_delta`, `agent.step_started`, `agent.subagent_started`, `agent.subagent_completed`, `agent.subagent_failed` |
| `tool` | `tool.call_started`, `tool.call_finished` |
| `artifact` | `artifact.created` |
| `log` | *(reserved)* |

### Event Rules

- Events are append-only (never mutated after emission)
- Payloads are flat JSON objects (no deeply nested structures)
- Field names in payloads use `snake_case`
- `task_id` is always included in the payload when relevant
- `run_id` is set on the `BusEvent` itself, not duplicated in the payload
- No meaningful model/tool transition may occur without an emitted event

### Batching and Immediacy

- Default event flush interval is 100ms with max batch size 50
- Immediate flush applies to interaction-critical events: `task.*`, `agent.step_*`, `agent.deciding`, `agent.tool_calls_preparing`
- Frontend event handlers must support both single-event and multi-event batches

### Dual Write Pattern

Events are both published to the broadcast channel AND persisted to the database:

```rust
pub fn emit_and_record(
    db: &Database,
    bus: &EventBus,
    category: &str,
    event_type: &str,
    run_id: Option<String>,
    payload: serde_json::Value,
) -> Result<BusEvent, String> {
    let event = bus.emit(category, event_type, run_id, payload);
    queries::insert_event(db, &EventRow { ... })?;
    Ok(event)
}
```

This ensures the UI can reconstruct state from events + database on crash recovery.

## Database Conventions

### Migration System

Migrations are versioned integers starting at 1. Each migration is a struct with a `version: i64` and `sql: &'static str`. New migrations append to the array — never modify existing ones:

```rust
const MIGRATIONS: &[Migration] = &[
    Migration { version: 1, sql: r#"CREATE TABLE tasks (...)"# },
    Migration { version: 2, sql: r#"CREATE INDEX ..."# },
];
```

### Schema Conventions

| Column Type | Convention |
|-------------|-----------|
| Primary keys | `TEXT` (UUID strings) |
| Foreign keys | `TEXT NOT NULL REFERENCES parent_table(id)` |
| Status columns | `TEXT NOT NULL DEFAULT 'pending'` |
| Timestamps | `TEXT NOT NULL` (RFC 3339 strings) |
| JSON blobs | `TEXT` columns suffixed with `_json` |
| Booleans | `INTEGER` (0/1) |
| Table names | `snake_case`, plural (`tasks`, `runs`, `sub_agents`) |
| Index names | `idx_{table}_{columns}` |

### Row Types

Each table has a corresponding `{Table}Row` struct in `queries.rs`:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct TaskRow {
    pub id: String,
    pub prompt: String,
    pub parent_task_id: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}
```

Row structs derive `Debug, Clone, Serialize` only (not `Deserialize`), since they are constructed in Rust from database reads.

### Query Functions

All database operations are free functions in `queries.rs` taking `&Database` as the first parameter:

```rust
pub fn insert_task(db: &Database, row: &TaskRow) -> Result<(), DbError> { ... }
pub fn list_tasks(db: &Database) -> Result<Vec<TaskRow>, DbError> { ... }
pub fn get_task(db: &Database, id: &str) -> Result<TaskRow, DbError> { ... }
pub fn update_task_status(db: &Database, id: &str, status: &str) -> Result<(), DbError> { ... }
pub fn delete_task(db: &Database, id: &str) -> Result<(), DbError> { ... }
```

Naming: `{verb}_{entity}` — e.g., `insert_run`, `list_tasks`, `update_task_status`, `get_latest_run_for_task`.

### SQLite Pragmas

SQLite is initialized with WAL mode and foreign keys enabled:

```rust
conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
```

## IPC Contract

The frontend and backend communicate through a strict contract:

1. **Frontend sends intents** via `invoke()` commands from `@tauri-apps/api/core`
2. **Backend emits state** via batched events on `orchestrix://events`
3. **Frontend never orchestrates** — it only renders and sends user actions
4. **Backend is authoritative** — all state, logic, and execution happen in Rust

### Command Invocations

```typescript
import { invoke } from "@tauri-apps/api/core";

const tasks = await invoke<TaskRow[]>("list_tasks");
await invoke("create_task", { prompt, options });
```

Command names match the Rust function names exactly (`snake_case`).

### Event Subscriptions

```typescript
import { listen } from "@tauri-apps/api/event";

const unlisten = await listen<BusEvent[]>("orchestrix://events", (event) => {
    // Process batch
});
```

### Type Mirroring

TypeScript interfaces in `types.ts` must mirror Rust row/event structs exactly (same field names in `snake_case`):

```typescript
export interface TaskRow {
    id: string;
    prompt: string;
    parent_task_id: string | null;
    status: TaskStatus;
    created_at: string;
    updated_at: string;
}
```

## File Organization

### Project Structure

```
orchestrix/
├── src/                          # Frontend (React + TypeScript)
│   ├── main.tsx                  # Entry point
│   ├── App.tsx                   # Root component
│   ├── index.css                 # Global styles (Tailwind v4)
│   ├── store.ts                  # Primary Zustand store
│   ├── streamStore.ts            # High-frequency reactive counters
│   ├── runtimeEventBuffer.ts     # Event-to-conversation mapper
│   ├── types.ts                  # Shared TypeScript interfaces
│   ├── lib/                      # Utilities and helpers
│   │   └── utils.ts              # cn() and general utilities
│   ├── layouts/                  # Layout shells
│   │   └── IdeShell.tsx
│   └── components/               # UI components
│       ├── ui/                   # Reusable primitives (button, input, etc.)
│       ├── Chat/                 # Chat feature components
│       │   ├── ChatInterface.tsx
│       │   ├── ConversationTimeline.tsx
│       │   └── hooks/            # Feature-scoped hooks
│       ├── Artifacts/            # Artifact review components
│       ├── Settings/             # Settings and configuration
│       ├── Header.tsx            # Titlebar
│       ├── Sidebar.tsx           # Conversation list
│       ├── Composer.tsx          # Input area
│       └── ErrorBoundary.tsx     # React error boundary
├── src-tauri/                    # Backend (Rust)
│   └── src/
│       ├── main.rs               # Thin entry (calls lib::run)
│       ├── lib.rs                # AppState, commands, run()
│       ├── bus/                  # Event bus and batching
│       ├── core/                 # Plan and tool descriptors
│       ├── db/                   # SQLite database layer
│       ├── model/                # LLM model clients (MiniMax, Kimi)
│       ├── policy/               # Permission and sandboxing engine
│       ├── runtime/              # Orchestrator, planner, recovery, worktree
│       ├── tools/                # Tool registry and built-in tools
│       └── testing.rs            # Unit and integration tests
└── AGENTS.md                     # Agent architecture documentation
```

### File Naming

| Type | Convention | Examples |
|------|-----------|----------|
| React components | PascalCase `.tsx` | `Sidebar.tsx`, `ChatInterface.tsx` |
| React hooks | camelCase prefixed with `use` | `useArtifactReview.ts` |
| Stores | camelCase | `store.ts`, `streamStore.ts` |
| Utilities | camelCase `.ts` | `utils.ts`, `runtimeEventBuffer.ts` |
| Types | camelCase `.ts` | `types.ts` |
| Rust source | snake_case `.rs` | `event_bus.rs`, `queries.rs` |
| Feature directories | PascalCase | `Chat/`, `Artifacts/`, `Settings/` |
| Module directories | snake_case | `bus/`, `core/`, `runtime/` |
| CSS files | kebab-case `.css` | `index.css` |

## Naming Conventions

### Variables and Functions

- Use camelCase in TypeScript: `isActive`, `handleClick`, `selectedTaskId`
- Use snake_case in Rust: `task_id`, `create_task`, `emit_and_record`
- Boolean variables should be prefixed: `is`, `has`, `should`, `can`
- Event handlers should be prefixed: `handle`

```tsx
// Good
const isAuthenticated = true;
const hasPermission = checkPermission();
const handleSubmit = () => { /* ... */ };

// Avoid
const authenticated = true;
const permission = checkPermission();
const submit = () => { /* ... */ };
```

### Types and Interfaces

- Use PascalCase for types and interfaces
- Prefer `interface` for object shapes, `type` for unions/primitives and props
- Suffix props types with `Props`
- Suffix row types with `Row`

```tsx
// Types (unions, primitives, props)
type TaskStatus = "pending" | "running" | "completed" | "failed";
type ComposerProps = { onSend: (prompt: string) => void };

// Interfaces (object shapes)
interface TaskRow {
    id: string;
    prompt: string;
    status: TaskStatus;
}
```

### Constants

Use `SCREAMING_SNAKE_CASE` for true constants in both TypeScript and Rust:

```tsx
const MAX_ITEMS_PER_TASK = 500;
const EMPTY_ARTIFACTS: readonly never[] = [];
```

```rust
const BUS_CAPACITY: usize = 1024;
const WORKER_MAX_TURNS: usize = 8;
```

### CSS Variables

Use kebab-case with double-dash prefix:

```css
--primary
--primary-foreground
--sidebar-border
--radius
```

## Testing Standards

### Test Organization

All Rust tests live in `src-tauri/src/testing.rs`, guarded by `#[cfg(test)]`:

```rust
// lib.rs
#[cfg(test)]
mod testing;
```

### Test Categories

**Unit tests** (run without external dependencies):
- Prefixed with `test_` followed by the module/function under test
- Examples: `test_policy_path_check`, `test_tool_registry`, `test_db_operations`

**Integration tests** (require API keys, marked with `#[ignore]`):
- Prefixed with `test_` followed by the scenario
- Run with `cargo test -- --ignored`
- Examples: `test_plan_generation_minimax`, `test_full_pipeline`

### Test Pattern

```rust
#[test]
fn test_feature_name() {
    // Setup
    let db = test_db();

    // Action
    let result = some_function(&db, args);

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap().field, expected);
}

#[tokio::test]
#[ignore] // Requires API key
async fn test_integration_scenario() {
    let api_key = std::env::var("MINIMAX_API_KEY").expect("set MINIMAX_API_KEY");
    // ...
}
```

### Test Helpers

Use shared test helpers for common setup:

```rust
fn test_db() -> Arc<Database> {
    Arc::new(Database::open_in_memory().expect("in-memory db"))
}
```

### Assertions

- `assert!()` for boolean conditions
- `assert_eq!()` for equality checks
- `.expect("descriptive message")` for unwrapping in tests

### Frontend Testing (Planned)

Frontend tests are not yet implemented. When added, they should use:
- **Vitest** as the test runner
- **Testing Library** for component tests
- Tests colocated with components in `__tests__/` directories or `*.test.tsx` files

## Linting and Formatting

### Rust

Run `cargo fmt` before committing. The project should use a `rustfmt.toml` in `src-tauri/`:

```toml
edition = "2021"
max_width = 100
tab_spaces = 4
use_field_init_shorthand = true
use_try_shorthand = true
```

Run `cargo clippy` to catch common issues:

```bash
cargo clippy -- -W clippy::all
```

### TypeScript

The project should adopt [Biome](https://biomejs.dev/) for linting and formatting. The existing code style uses:
- 2-space indentation
- Double quotes
- Semicolons
- Trailing commas
- ~120 character line width

### TypeScript Strictness

These `tsconfig.json` settings must remain enabled:

```json
{
    "compilerOptions": {
        "strict": true,
        "noUnusedLocals": true,
        "noUnusedParameters": true,
        "noFallthroughCasesInSwitch": true
    }
}
```

## Comments and Documentation

### When to Comment

- **Do**: Explain *why*, not *what*
- **Do**: Document complex algorithms or business logic
- **Do**: Add JSDoc for public APIs and utilities
- **Don't**: State the obvious
- **Don't**: Comment bad code — refactor it instead

```tsx
// Good - explains why
// Collapse sidebar if clicking the already-active item
if (activeActivity === activity && !isCollapsed) {
    panel.collapse();
}

// Avoid - states the obvious
// Set the state to true
setState(true);
```

### Rust Documentation

Use `///` for public doc comments, `//` for inline comments, and section dividers for long files:

```rust
// ---------------------------------------------------------------------------
// Row types -- flat structs that map directly to table columns
// ---------------------------------------------------------------------------
```

Module-level documentation uses `//!`:

```rust
//! Shared helpers used by both MiniMax and Kimi model implementations.
```

### JSDoc for Public APIs

```tsx
/**
 * RuntimeEventBuffer -- converts raw BusEvents into structured conversation items.
 * Maintains a mapping of run IDs to task IDs for event routing.
 */
```

### TODOs and FIXMEs

Use standardized comment tags:

```tsx
// TODO: Add keyboard navigation support
// FIXME: Handle edge case when panel is already collapsed
// NOTE: This workaround is needed due to Tauri event serialization
```

## Additional Guidelines

### Imports

Order imports logically:

1. External packages
2. Internal absolute imports (using `@/`)
3. Relative imports
4. Type-only imports (if separated)

```tsx
// External
import { useState, useEffect } from "react";
import { useShallow } from "zustand/shallow";

// Internal absolute
import { useAppStore } from "@/store";
import { Button } from "@/components/ui/button";

// Relative
import { UserMessage } from "./UserMessage";

// Type-only
import type { TaskRow } from "@/types";
```

### Package Management

**Always use Bun.** This is configured in `tauri.conf.json`. Never use npm, yarn, or pnpm:

```bash
# Good
bun add package-name
bun remove package-name
bun install

# Never
npm install package-name
```

### Security

- All tool invocations are evaluated against the `PolicyEngine` (path sandboxing, command allowlist)
- All tool calls are audited and persisted
- Agents cannot escalate permissions or access outside the workspace scope
- CSP is configured in `tauri.conf.json` — review before modifying

---

Following these standards ensures a consistent, maintainable, and high-quality codebase. When in doubt, look at existing code in the project for reference, favoring the patterns documented here when they differ from the current proof-of-concept state.
