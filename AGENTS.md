# AGENTS.md

This document defines the agent architecture, roles, execution model, and design constraints for the **Tauri-based lightweight AI agent desktop app**.  
The app is optimized for low overhead, deterministic execution, and extensibility.

---

## Important NOTES

- **Delete then re-write**: When re-writing a file, delete it first, then write the new file, instead of trying to replace it's content.
- **YAGNI with backbone**: Do not write code for hypothetical future needs. Implement only what is required now. Build scalable, maintainable foundations without dead code—extensibility comes from clean architecture, not speculative utilities. You can always add it later when the need is real.

---

## Core Principles

- **Backend-authoritative**: All orchestration, state, and execution live in the Rust backend.
- **Event-driven UI**: Frontend renders state via streamed events; it never controls logic.
- **Conversation-first execution**: Agent runs as a natural coding conversation loop and decides tools dynamically.
- **Human-in-the-loop by default**: User review, approval, and intervention points are first-class.
- **Transparency-first UX**: Users can inspect decisions, tool activity, and artifacts throughout a run.
- **Condensed, non-cluttered visualization**: Show high-signal summaries by default with expandable detail.
- **Minimal surface area**: Humans view and inspect code and artifacts; they do not directly orchestrate execution or invoke tools.
- **Model-agnostic by design**: MiniMax, Kimi, and GLM (Z.AI/Zhipu) are supported through the same planner/worker interfaces.

---

## Agent Model

Agents are isolated execution contexts with:
- Independent message history
- Scoped tool permissions
- Independent budgets (tokens, time, tools)
- Deterministic inputs and outputs

---

## Agent Roles (Current)

### 1. Plan-Mode Agent (Planning)
**Model**: Same provider as worker (MiniMax, Kimi, or GLM/Zhipu).

**Responsibilities**
- Run in a **multi-turn loop** (like the worker): decide → tool calls → execute tools → decide again.
- Use **plan-mode tools** only: read-only (e.g. `fs.list`, `fs.read`, `search.rg`, `git.*`), plus `agent.create_artifact` and `agent.request_build_mode`.
- Explore the workspace autonomously before submitting a plan.
- **Submit the plan** by calling `agent.create_artifact` with the plan markdown (filename e.g. `plan.md`, kind `plan`, content = full markdown). The orchestrator does **not** auto-create an artifact from the first message; the plan is only finalized when the model calls `agent.create_artifact`.

**Constraints**
- No write/exec tools in plan mode; plan output is only via `agent.create_artifact`.

### 2. Worker Agent (Build Mode)
**Model**: Provider-configurable (for example `MiniMax-M2.5`, `kimi-k2.5`, or `glm-5`)

**Responsibilities**
- Execute user intent directly through a conversational tool-use loop
- Invoke tools via the tool layer
- Emit progress, logs, and artifacts
- Stop when work is complete or user cancels

**Constraints**
- Cannot escalate permissions

---

## Task Lifecycle

1. **Task Created**
   - User submits a request
   - Task persisted to local database

2. **Plan Phase** (plan mode)
   - User starts planning (e.g. "Plan" or "Generate plan").
   - Plan-mode agent runs in a **multi-turn loop**: can use read-only tools (fs.list, fs.read, search.rg, etc.) to explore the workspace, then must call `agent.create_artifact` to submit the plan. No artifact is created by the orchestrator until the model does so.
   - Events: `agent.planning_started`, `agent.deciding`, `agent.tool_calls_preparing`, `tool.call_started` / `tool.call_finished`, then `artifact.created` and `agent.plan_ready` when the plan is submitted.
   - Task moves to `awaiting_review`; user can approve or send feedback.

3. **Execution Phase** (build mode)
   - After plan approval, worker agent executes in a natural conversational loop.
   - Worker may call tools one or many at a time (native tool calling where provider supports it).
   - Tools invoked via permission-gated layer.
   - Events streamed to UI with progressive disclosure (summary first, full detail on demand).
   - User remains involved with live visibility and can cancel at any time.

---

## Sub-Agents

Sub-agents are delegated execution units.

Canonical behavior is defined in `SUB_AGENTS_SPEC.md`.

Non-negotiable rules:
- Sub-agents run via explicit delegation contracts (bounded context, tools, runtime)
- No shared mutable memory across agent boundaries
- Parent owns integration, final status, and closure of every child
- Worker completion is not final completion; post-join integration gates run outcome

Lifecycle:
- `created -> running -> waiting_for_merge -> completed|failed -> closed`

Sub-agents are intended for:
- Clearly parallelizable, low-conflict steps
- Specialized tasks (tests, analysis, scaffolding)

---

## Tools & Skills

### Tool Layer (v1 Built-ins)

- Filesystem (read/write, scoped)
- File search (ripgrep-style)
- Command execution (sandboxed, gated)
- Git status / diff / patch apply

### Permissions

Each tool invocation is evaluated against:
- Workspace scope
- Command allowlist
- Network access policy
All tool calls are audited.

---

## MCP Compatibility

The tool interface is designed to be **MCP-compatible by default**.

- Tools map 1:1 with MCP tool definitions
- Future skills are expected to be implemented as MCP servers
- The app acts as an MCP client

This allows:
- Reusable skills
- External tool providers
- No hardcoded integrations

---

## Event System

Agents never communicate directly with the UI.

All communication happens via events emitted by the backend.

### Event Categories

- `task.*`
- `agent.*`
- `tool.*`
- `log.*`
- `artifact.*`

### Event Rules

- Events are append-only
- High-frequency events are batched
- No meaningful agent/tool transition may occur without a corresponding event
- UI must be able to reconstruct state from events + DB

### Event catalog (immediate vs batched)

Events for which the batcher flushes immediately (no 100ms delay): `task.*`, `agent.step_*`, `agent.deciding`, `agent.tool_calls_preparing`. All other events are batched.

**UX feedback events** (so the user sees the AI is not frozen):

- `agent.deciding` — emitted at the start of each worker turn before the model is called; payload: `task_id`, `run_id`, `step_idx`, `sub_agent_id`, `turn`. Frontend shows "Thinking…".
- `agent.tool_calls_preparing` — emitted when the model has returned tool calls but before any `tool.call_started`; payload: `task_id`, `run_id`, `tool_names[]`, `step_idx`, `sub_agent_id`. Frontend shows "Preparing: fs.write, …".
- `agent.message_stream_started` — emitted when the worker begins streaming assistant text; payload: `task_id`, `sub_agent_id`, `step_idx`, `turn`, `stream_id`.
- `agent.message_delta` — emitted for incremental assistant text chunks; payload: `task_id`, `sub_agent_id`, `step_idx`, `turn`, `stream_id`, `content`.
- `agent.message_stream_completed` — emitted after final text chunk; payload: `task_id`, `sub_agent_id`, `step_idx`, `turn`, `stream_id`.
- `agent.message_stream_cancelled` — emitted when a partial stream is discarded (for example, provider switches to tool-calling output); payload: `task_id`, `sub_agent_id`, `step_idx`, `turn`, `stream_id`, `reason`.

**Planning (plan mode) events:**

- `agent.planning_started` — plan generation began; payload: `task_id`. Frontend shows "Generating execution plan…".
- `agent.deciding` — each planning turn (same as worker); payload includes `task_id`, `run_id`, `turn`. Plan mode runs a multi-turn loop until the model calls `agent.create_artifact`.
- `agent.tool_calls_preparing` — plan-mode tool calls (e.g. fs.list, fs.read, then agent.create_artifact); payload: `task_id`, `run_id`, `tool_names[]`, `turn`.
- `tool.call_started` / `tool.call_finished` — tool executions during planning (read-only tools + agent.create_artifact).
- `agent.plan_message` — assistant message (e.g. "I drafted a plan…"); payload: `task_id`, `content`. Shown in the chat timeline.
- `agent.plan_ready` — structured plan is available after the model calls `agent.create_artifact`; payload: `task_id`, `plan`: `{ goal_summary, steps: [{ title, description }], completion_criteria? }`. Parsed from the plan markdown so the UI can show goal and step list inline.
- `artifact.created` — emitted when the plan artifact is written (content comes from `agent.create_artifact`; the orchestrator does not create an artifact from the first model message).

Plan mode is **multi-turn**: the agent may use tools to explore the workspace, then must submit the plan by calling `agent.create_artifact`. Only then is the plan artifact written and `agent.plan_ready` emitted.

**Artifact events:** `artifact.created` may optionally include `content` or `content_preview` for UI preview.

---

## Persistence

All tasks and runs are persisted locally (SQLite):

- Tasks
- Task links
- Runs
- Sub-agents
- Events
- Tool calls
- Artifacts
- Checkpoints
- Worktree logs
- Settings

Crash recovery is mandatory.

---

## UI Implementation Standards

### Tech Stack

- **Tailwind CSS v4**: Primary styling framework using the new `@theme` directive and OKLCH colors
- **shadcn/ui**: Component library built on Radix UI primitives (use `bunx shadcn add <component>`)
- **Lucide Icons**: Icon library (already configured in components.json)

### Required Patterns

**1. Prefer shadcn/ui Components**
- Always check if a shadcn component exists before building custom UI
- Install with: `bunx shadcn add <component>`
- Common components: `popover`, `command`, `dropdown-menu`, `tooltip`, `tabs`, `switch`, `badge`, `card`, `separator`, `scroll-area`, `dialog`

**2. Custom Component Guidelines**
- When custom components are needed, use shadcn primitives internally where applicable:
  - Use `<Popover>` for floating panels (e.g., ContextUsagePopover)
  - Use `<Command>` + `<Combobox>` for searchable dropdowns
  - Use `<Tooltip>` for icon-only buttons
- Wrap shadcn components rather than rebuilding from scratch
- Maintain `data-slot` attributes for styling hooks (shadcn v4 standard)

**3. Tailwind v4 Standards**
- Use `@theme inline` for custom CSS variables (already in `src/index.css`)
- Use OKLCH color format for new variables
- Leverage dynamic spacing utilities (no arbitrary values needed in v4)
- Avoid `@layer base` for variables; use `:root` + `@theme`

**4. Design System Compliance**
- All UI must align with `DESIGN_SYSTEM.md`:
  - Use semantic tokens (`primary`, `muted`, `accent`, `success`, `warning`, `info`, `destructive`)
  - 8pt grid system (spacing divisible by 4 or 8)
  - 4 font sizes, 2 weights (semibold, regular)
  - 60/30/10 color distribution
  - Professional, minimal aesthetic

**5. Component Architecture**
- Use CVA (class-variance-authority) for variant styling (shadcn standard)
- Keep components in `src/components/ui/` for primitives
- Use `cn()` helper from `@/lib/utils` for class merging
- Add `data-slot` attributes to component root elements

### Anti-Patterns to Avoid

- ❌ Building custom dropdowns when `<DropdownMenu>` exists
- ❌ Using native `<select>` when `<Select>` or `<Combobox>` is available
- ❌ Creating popovers from scratch instead of using `<Popover>`
- ❌ Hardcoding colors instead of using CSS variables
- ❌ Creating one-off components that could use shadcn primitives

---

## Frontend Contract

The frontend:
- Subscribes to events
- Renders task state
- Sends user intents (start/stop, task CRUD, settings updates)

The frontend:
- Does **not** orchestrate agents
- Does **not** modify execution plans
- Does **not** invoke tools directly

---

## Non-Goals

- No opaque autonomous execution
- No silent system-level side effects
- No leaving the user out of what is happening
- No redundant UI for chat interfaces
- No step-runner UI that hides natural agent messages or obscures tool activity

---

## Design Philosophy

This system prioritizes:
- Artifact based planning
- Context-aware agentic management
- Transparency over magic
- Composition over monoliths

Agents are **tools**, not replacements for intent.

---

## References

### Documentation
- [ARCHITECTURE.md](./ARCHITECTURE.md) - System architecture and component overview
- [DESIGN_SYSTEM.md](./DESIGN_SYSTEM.md) - Visual design tokens and UI standards
- [UX_PRINCIPLES.md](./UX_PRINCIPLES.md) - UX, transparency, and performance guardrails
- [SETUP.md](./SETUP.md) - Development environment setup
- [CODING_STANDARDS.md](./CODING_STANDARDS.md) - Code conventions and standards

### Skills
- **orchestrix-app-development** - Use when implementing Orchestrix features (see `.agents/skills/orchestrix-app-development/SKILL.md`)

---

## IMPORTANT NOTES

ALWAYS USE BUN. DO NOT EVER USE NPM, PNPM, OR ANY OTHER PACKAGE MANAGER BESIDES BUN.