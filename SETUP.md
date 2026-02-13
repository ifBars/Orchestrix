# Setup Guide

Complete instructions for setting up the Orchestrix development environment.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Configuration](#configuration)
- [Development Workflow](#development-workflow)
- [Platform-Specific Notes](#platform-specific-notes)
- [Verification](#verification)

## Prerequisites

### Required Software

| Tool | Minimum Version | Purpose |
|------|----------------|---------|
| [Rust](https://rustup.rs/) | 1.75+ | Backend compilation |
| [Bun](https://bun.sh/) | 1.0+ | Package management (required) |
| [Git](https://git-scm.com/) | 2.30+ | Version control |

### System Requirements

- **OS**: Windows 10/11, macOS 12+, or Linux
- **RAM**: 4GB minimum, 8GB recommended
- **Disk**: 2GB free space

### API Keys

You need an API key from at least one of these providers:

- **MiniMax**: [platform.minimaxi.com](https://platform.minimaxi.com/)
- **Kimi**: [platform.moonshot.cn](https://platform.moonshot.cn/)

## Installation

### Step 1: Install Rust

```bash
# Using rustup (recommended)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Or on Windows with winget
winget install Rustlang.Rustup

# Verify installation
rustc --version  # Should show 1.75 or higher
```

### Step 2: Install Bun

**Important**: Orchestrix requires Bun. Do not use npm, pnpm, or yarn.

```bash
# macOS/Linux
curl -fsSL https://bun.sh/install | bash

# Windows (PowerShell as Administrator)
powershell -c "irm bun.sh/install.ps1 | iex"

# Verify installation
bun --version
```

### Step 3: Clone Repository

```bash
git clone <repository-url>
cd orchestrix
```

### Step 4: Install Dependencies

```bash
bun install
```

This installs all frontend dependencies and Tauri CLI.

## Configuration

### API Key Setup

Choose one of these methods:

#### Method 1: Environment Variables (Recommended for Development)

```bash
# Add to ~/.bashrc, ~/.zshrc, or Windows Environment Variables

# For MiniMax
export MINIMAX_API_KEY="your-minimax-api-key"
export MINIMAX_MODEL="MiniMax-M2.1"  # optional
export MINIMAX_BASE_URL="https://api.minimaxi.chat"  # optional

# For Kimi
export KIMI_API_KEY="your-kimi-api-key"
export KIMI_MODEL="kimi-for-coding"  # optional
export KIMI_BASE_URL="https://api.moonshot.cn"  # optional
```

#### Method 2: Application Settings

1. Start the application: `bun tauri dev`
2. Open Settings (gear icon in top right)
3. Navigate to "Providers" tab
4. Enter your API key and configure model settings

### Workspace Configuration

By default, Orchestrix uses the current working directory as the workspace. To change:

```bash
# Set custom workspace
export ORCHESTRIX_WORKSPACE="/path/to/your/project"
```

Or use the Settings UI:
1. Open Settings
2. Go to "Workspace" tab
3. Select new workspace folder

### Database Location

The SQLite database is stored at:

- **Windows**: `%APPDATA%/Orchestrix/orchestrix.db`
- **macOS**: `~/.orchestrix/orchestrix.db`
- **Linux**: `~/.orchestrix/orchestrix.db`

To override:

```bash
export ORCHESTRIX_DATA_DIR="/custom/path"
```

## Development Workflow

### Starting Development Server

```bash
# Start with hot reload
bun tauri dev
```

This command:
1. Starts the Vite dev server for frontend
2. Compiles and runs the Rust backend
3. Opens the Tauri desktop window
4. Enables hot reload for both frontend and backend

### Common Commands

```bash
# Frontend only (Vite dev server)
bun run dev

# Type check TypeScript
bun run tsc

# Build frontend for production
bun run build

# Build Tauri application
bun tauri build

# Update Tauri dependencies
bun tauri update
```

### Rust Development

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Run tests
cargo test

# Run integration tests (requires API keys)
cargo test -- --ignored

# Build release
cargo build --release
```

### Debugging

The application uses `tracing` for structured logging. Set log level:

```bash
# Debug logging
RUST_LOG=orchestrix=debug,info bun tauri dev

# Trace logging (verbose)
RUST_LOG=orchestrix=trace bun tauri dev
```

## Platform-Specific Notes

### Windows

- Install Visual Studio Build Tools 2022 with "Desktop development with C++" workload
- Enable Developer Mode in Windows Settings for symlink support
- Use PowerShell or Git Bash (not Command Prompt)

### macOS

- Install Xcode Command Line Tools: `xcode-select --install`
- Grant necessary permissions for app execution in System Preferences

### Linux

Install system dependencies:

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install libwebkit2gtk-4.0-dev \
    build-essential \
    curl \
    wget \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev

# Fedora
sudo dnf install webkit2gtk4.0-devel \
    openssl-devel \
    curl \
    wget \
    libappindicator-gtk3-devel \
    librsvg2-devel

# Arch Linux
sudo pacman -S webkit2gtk \
    base-devel \
    curl \
    wget \
    openssl \
    appmenu-gtk-module \
    libappindicator-gtk3 \
    librsvg
```

## Verification

After setup, verify everything works:

1. **Start the application**
   ```bash
   bun tauri dev
   ```
   The window should open without errors.

2. **Check API connection**
   - Go to Settings → Providers
   - Verify your provider shows as "Configured"

3. **Create a test task**
   - Click "New Task"
   - Enter: "Hello world"
   - You should see the AI generate a plan and wait for review

4. **Validate human-in-the-loop UX**
   - Review the plan before approval
   - Approve and confirm execution events appear in the timeline
   - Expand a tool/event row and verify detailed payload visibility
   - Confirm the timeline remains condensed (summary first, details on demand)

## Troubleshooting Setup

### Bun not found

Make sure Bun is in your PATH:
```bash
# macOS/Linux - add to ~/.bashrc or ~/.zshrc
export PATH="$HOME/.bun/bin:$PATH"

# Windows - add to System Environment Variables
# C:\Users\<username>\.bun\bin
```

### Rust compilation errors

Ensure you're using the correct Rust version:
```bash
rustup update
rustup default stable
```

### Tauri build fails

Clear caches and rebuild:
```bash
rm -rf src-tauri/target
rm -rf node_modules
bun install
bun tauri dev
```

### API key not recognized

Check environment variables are loaded:
```bash
# Linux/macOS
echo $MINIMAX_API_KEY

# Windows PowerShell
$env:MINIMAX_API_KEY
```

For more issues, see [TROUBLESHOOTING.md](./TROUBLESHOOTING.md).

## Next Steps

1. Read [ARCHITECTURE.md](./ARCHITECTURE.md) to understand the system
2. Review [UX_PRINCIPLES.md](./UX_PRINCIPLES.md) for UX, transparency, and performance guardrails
3. Review [DESIGN_SYSTEM.md](./DESIGN_SYSTEM.md) for visual design tokens and UI standards
4. Review [AGENTS.md](./AGENTS.md) for agent architecture and execution model
5. Review [CODING_STANDARDS.md](./CODING_STANDARDS.md) for code conventions
6. Check out the [examples/](./examples/) directory for sample code
7. Start developing! Create a new task and explore the codebase.

## References

### Documentation
- [AGENTS.md](./AGENTS.md) - Agent architecture and execution model
- [ARCHITECTURE.md](./ARCHITECTURE.md) - System architecture and data flow
- [DESIGN_SYSTEM.md](./DESIGN_SYSTEM.md) - Visual design tokens and UI standards
- [UX_PRINCIPLES.md](./UX_PRINCIPLES.md) - UX, transparency, and performance guardrails
- [CODING_STANDARDS.md](./CODING_STANDARDS.md) - Code conventions and standards

### Skills
- **orchestrix-app-development** - Use when implementing Orchestrix features (see `.agents/skills/orchestrix-app-development/SKILL.md`)
