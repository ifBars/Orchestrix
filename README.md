# Orchestrix

A lightweight, desktop AI agent management application built with Tauri, Rust, and React. Orchestrate AI agents to work on coding tasks, manage workflows, and automate development workflows.

## Overview

Orchestrix is a **backend-authoritative, event-driven** desktop application designed for managing AI agents. It provides:

- **Conversation-first execution**: Natural chat interface for task management
- **Multi-provider support**: Works with MiniMax and Kimi models
- **Plan-then-execute workflow**: AI plans tasks before execution with human-in-the-loop review
- **Sub-agent delegation**: Parallel task execution via specialized sub-agents
- **Tool-based operations**: File system, command execution, git operations, and more
- **Full audit trail**: All events and tool calls persisted to SQLite

## Key Features

### Agent Management
- Create and manage AI tasks through natural conversation
- Plan review and approval before execution
- Real-time progress monitoring via event streaming
- Sub-agent delegation for parallel work

### Model Support
- **MiniMax**: MiniMax-M2.1 and other models
- **Kimi**: kimi-k2.5 and other coding-optimized models
- Easy provider configuration via UI or environment variables

### Tool System
- File system operations (read, write, search)
- Command execution with sandboxing
- Git operations and worktree management
- Skill-based extensibility (MCP-compatible)

### Workspace Management
- Configurable workspace root
- Git worktree isolation for sub-agents
- Artifact generation and review
- Conflict detection and resolution

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) 1.75+
- [Bun](https://bun.sh/) (required - do not use npm/pnpm)
- API key from [MiniMax](https://www.minimaxi.com/) or [Kimi](https://platform.moonshot.cn/)

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd orchestrix

# Install dependencies
bun install

# Configure API key (choose one)
export MINIMAX_API_KEY="your-key-here"
# OR
export KIMI_API_KEY="your-key-here"

# Start the development server
bun tauri dev
```

### First Task

1. Open the application
2. Click "New Task" in the sidebar
3. Enter a prompt (e.g., "Create a Python script that fetches weather data")
4. Review the generated plan
5. Click "Approve" to start execution
6. Monitor progress in the conversation timeline

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        Frontend                             │
│                   (React + TypeScript)                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   Sidebar    │  │    Chat      │  │  Artifacts   │      │
│  │  (Task List) │  │ (Timeline)   │  │   (Review)   │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└────────────────────┬────────────────────────────────────────┘
                     │ invoke / events
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                      Backend (Rust)                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  Tauri Cmds  │  │ Orchestrator │  │   Planner    │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   Tools      │  │  Event Bus   │  │  Worktrees   │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
              ┌──────────────┐
              │   SQLite     │
              │ (Tasks, Runs,│
              │  Events,     │
              │  Artifacts)  │
              └──────────────┘
```

### Core Principles

- **Backend-authoritative**: All orchestration, state, and execution live in Rust
- **Event-driven UI**: Frontend renders state via streamed events; never controls logic
- **Plan-first execution**: Every task begins with a structured planning phase
- **Minimal surface area**: No embedded editor, no live code manipulation by humans

## Documentation

- **[SETUP.md](./SETUP.md)** - Detailed installation and configuration
- **[ARCHITECTURE.md](./ARCHITECTURE.md)** - System design and data flow
- **[CODING_STANDARDS.md](./CODING_STANDARDS.md)** - Code style and conventions
- **[AGENTS.md](./AGENTS.md)** - Agent architecture and execution model
- **[SKILLS_GUIDE.md](./SKILLS_GUIDE.md)** - Working with the skills system
- **[TROUBLESHOOTING.md](./TROUBLESHOOTING.md)** - Common issues and solutions

## Project Structure

```
orchestrix/
├── src/                    # Frontend (React + TypeScript)
│   ├── components/         # React components
│   ├── stores/             # Zustand state management
│   ├── types/              # TypeScript type definitions
│   └── lib/                # Utilities
├── src-tauri/              # Backend (Rust)
│   └── src/
│       ├── commands/       # Tauri command handlers
│       ├── runtime/        # Orchestrator, planner, recovery
│       ├── db/             # Database layer
│       ├── model/          # LLM clients
│       ├── tools/          # Tool registry
│       └── bus/            # Event bus and batching
└── .agents/skills/         # Agent skills (MCP-compatible)
```

## Development

### Scripts

```bash
# Development
bun tauri dev           # Start dev server with hot reload

# Building
bun run build           # Build frontend
bun tauri build         # Build production app

# Code Quality
cargo fmt               # Format Rust code
cargo clippy            # Lint Rust code
```

### Tech Stack

**Frontend:**
- React 19 + TypeScript
- Tailwind CSS v4 with OKLCH colors
- Zustand for state management
- shadcn/ui components

**Backend:**
- Rust + Tauri v2
- SQLite with rusqlite
- tokio for async runtime
- MiniMax/Kimi API clients

## Contributing

1. Follow [CODING_STANDARDS.md](./CODING_STANDARDS.md)
2. Write tests for new features in `src-tauri/src/testing.rs`
3. Use `bun` for all package management
4. Ensure `cargo fmt` and `cargo clippy` pass

## License

[Add your license here]

---

**Note:** This project uses Bun exclusively. Do not use npm, pnpm, or yarn.
