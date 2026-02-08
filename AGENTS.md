# AGENTS.md

This document defines the agent architecture, roles, execution model, and design constraints for the **Tauri-based lightweight AI agent desktop app**.  
The app is **agent-management–only** (no human code editing) and is optimized for low overhead, deterministic execution, and extensibility.

---

## Core Principles

- **Backend-authoritative**: All orchestration, state, and execution live in the Rust backend.
- **Event-driven UI**: Frontend renders state via streamed events; it never controls logic.
- **Conversation-first execution**: Agent runs as a natural coding conversation loop and decides tools dynamically.
- **Minimal surface area**: No embedded editor, no live code manipulation by humans.
- **Model-agnostic by design**: MiniMax and Kimi are both supported through the same planner/worker interfaces.

---

## Agent Model

Agents are isolated execution contexts with:
- Independent message history
- Scoped tool permissions
- Independent budgets (tokens, time, tools)
- Deterministic inputs and outputs

Agents do **not** directly manipulate UI or global state.

---

## Agent Roles (Current)

### 1. Worker Agent
**Model**: Provider-configurable (`MiniMax-M2.1` or `kimi-k2.5` by default)

**Responsibilities**
- Execute user intent directly through a conversational tool-use loop
- Invoke tools via the tool layer
- Emit progress, logs, and artifacts
- Stop when work is complete or user cancels

**Constraints**
- Cannot escalate permissions

---

## Future Agent Roles (Not Implemented)

### Reviewer Agent
- Validates artifacts (tests, outputs, structure)
- Verifies completion criteria
- Flags failures or regressions

### Integrator Agent
- Combines outputs from multiple workers
- Produces final artifact set
- Prepares summary result

---

## Task Lifecycle

1. **Task Created**
   - User submits a request
   - Task persisted to local database

2. **Execution Phase**
   - Worker agent executes user request(s) in a natural conversational loop
   - Worker may call tools one or many at a time (native tool calling where provider supports it)
   - Tools invoked via permission-gated layer
   - Events streamed to UI

3. **Completion**
   - Task marked as `completed` or `failed`
   - Artifacts & chat finalized and persisted

---

## Sub-Agents

Sub-agents are planner-defined delegated execution units.

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
- Explicit approval gates (future)

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
- UI must be able to reconstruct state from events + DB

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
- No "executing steps" more like a natural coding agent, that can make todo lists with tools, but talks to the user with natural language, and we dont shove random UI in their to hide AI messages

---

## Design Philosophy

This system prioritizes:
- Artifact based planning
- Context-aware agentic management
- Transparency over magic
- Composition over monoliths

Agents are **tools**, not replacements for intent.

---

## Versioning

This document applies to:
- Agent System
- Multi-provider execution (MiniMax + Kimi)
- No external MCP servers yet

Future revisions must preserve backward compatibility where possible.

---

## IMPORTANT NOTES

ALWAYS USE BUN. DO NOT EVER USE NPM, PNPM, OR ANY OTHER PACKAGE MANAGER BESIDES BUN.

When re-writing a file, delete it first, then write the new file, instead of trying to replace it's content.
