# Troubleshooting Guide

Common issues and solutions when working with Orchestrix.

## Table of Contents

- [Build Issues](#build-issues)
- [Runtime Issues](#runtime-issues)
- [API/Model Issues](#apimodel-issues)
- [Database Issues](#database-issues)
- [Frontend Issues](#frontend-issues)
- [UX and Transparency Issues](#ux-and-transparency-issues)
- [Development Issues](#development-issues)
- [Getting Help](#getting-help)

## Build Issues

### Bun Install Fails

**Error**: `bun: command not found` or installation errors

**Solutions**:

```bash
# Verify Bun installation
bun --version

# If not found, add to PATH
# macOS/Linux: Add to ~/.bashrc or ~/.zshrc
export PATH="$HOME/.bun/bin:$PATH"

# Windows: Add to System Environment Variables
# C:\Users\<username>\.bun\bin

# Reinstall Bun if needed
curl -fsSL https://bun.sh/install | bash
```

### Rust Compilation Errors

**Error**: Various compilation errors in `src-tauri/`

**Solutions**:

```bash
# Update Rust to latest version
rustup update
rustup default stable

# Clean build artifacts
cd src-tauri
cargo clean
cd ..

# Delete node_modules and reinstall
rm -rf node_modules
bun install

# Rebuild
bun tauri dev
```

### Tauri Build Fails on Windows

**Error**: `link.exe not found` or Windows SDK errors

**Solution**:

1. Install Visual Studio Build Tools 2022
2. Select "Desktop development with C++" workload
3. Ensure Windows SDK is installed
4. Enable Developer Mode in Windows Settings

### Out of Memory During Build

**Error**: `memory allocation failed` or system freeze

**Solutions**:

```bash
# Increase Node memory limit (for frontend)
export NODE_OPTIONS="--max-old-space-size=4096"

# Use release mode for faster builds
cd src-tauri
cargo build --release

# Close unnecessary applications
# Consider upgrading RAM if persistent
```

## Runtime Issues

### Application Won't Start

**Symptom**: Window doesn't open or crashes immediately

**Solutions**:

```bash
# Check for database lock
rm -f orchestrix.db orchestrix.db-journal

# Reset to default workspace
unset ORCHESTRIX_WORKSPACE

# Check database permissions
ls -la ~/.orchestrix/orchestrix.db

# Run with verbose logging
RUST_LOG=debug bun tauri dev
```

### White Screen / Blank Window

**Symptom**: Application window opens but is blank

**Solutions**:

1. Check frontend console for errors (F12 in development)
2. Verify frontend built successfully:
   ```bash
   bun run build
   ```
3. Check for TypeScript errors:
   ```bash
   bun run tsc
   ```

### Database Connection Failed

**Error**: `failed to open database` or `database is locked`

**Solutions**:

```bash
# Kill any zombie processes
pkill -f orchestrix

# Remove lock file (if safe - no other instances running)
rm -f ~/.orchestrix/orchestrix.db-journal
rm -f ~/.orchestrix/orchestrix.db-shm
rm -f ~/.orchestrix/orchestrix.db-wal

# Or use in-memory database for testing
export ORCHESTRIX_DATA_DIR=/tmp/orchestrix-test
```

### Crash on Startup

**Symptom**: Application crashes immediately with stack trace

**Solutions**:

```bash
# Check for migration issues
RUST_LOG=orchestrix=debug bun tauri dev

# Reset database (WARNING: data loss)
rm -rf ~/.orchestrix/

# Check system compatibility
rustc --version  # Should be 1.75+
bun --version    # Should be 1.0+
```

## API/Model Issues

### API Key Not Recognized

**Symptom**: Provider shows as "Not configured" in settings

**Solutions**:

```bash
# Verify environment variables
echo $MINIMAX_API_KEY  # or $KIMI_API_KEY

# Set in current session
export MINIMAX_API_KEY="sk-..."

# Or set in shell profile
echo 'export MINIMAX_API_KEY="sk-..."' >> ~/.bashrc

# Restart the application after setting
```

### Plan Generation Fails

**Symptom**: "Planning failed" error or timeout

**Solutions**:

1. Check API key validity:
   ```bash
   curl -H "Authorization: Bearer $MINIMAX_API_KEY" \
        https://api.minimaxi.chat/v1/models
   ```

2. Check network connectivity
3. Verify API key has sufficient credits
4. Check model availability:
   ```bash
   # Try a different model
   export MINIMAX_MODEL="MiniMax-M1"
   ```

### Task Execution Hangs

**Symptom**: Task stays in "executing" state indefinitely

**Solutions**:

```bash
# Cancel the task
# In UI: Click cancel button on task

# Check logs for errors
RUST_LOG=orchestrix=trace bun tauri dev

# Restart if needed
# Tasks will be recovered on restart
```

### "No configured provider" Error

**Symptom**: Cannot create tasks, provider error

**Solutions**:

1. Set at least one provider API key:
   ```bash
   export MINIMAX_API_KEY="your-key"
   # OR
   export KIMI_API_KEY="your-key"
   ```

2. Or configure via UI:
   - Open Settings → Providers
   - Enter API key for your preferred provider

## Database Issues

### Migration Errors

**Error**: `migration failed` on startup

**Solutions**:

```bash
# Check current schema version
cd ~/.orchestrix
sqlite3 orchestrix.db "SELECT version FROM schema_migrations;"

# Backup database first
cp orchestrix.db orchestrix.db.backup

# Reset to fresh database (WARNING: data loss)
rm orchestrix.db

# Application will recreate with latest schema
```

### Database Locked

**Error**: `database is locked` or `busy`

**Solutions**:

```bash
# Find and kill processes using the database
lsof ~/.orchestrix/orchestrix.db

# On Windows
handle.exe orchestrix.db

# Wait for WAL checkpoint
sqlite3 ~/.orchestrix/orchestrix.db "PRAGMA wal_checkpoint;"

# Or disable WAL mode temporarily
sqlite3 ~/.orchestrix/orchestrix.db "PRAGMA journal_mode=DELETE;"
```

### Corrupted Database

**Symptom**: SQLite errors, missing tables

**Solutions**:

```bash
# Check database integrity
sqlite3 ~/.orchestrix/orchestrix.db "PRAGMA integrity_check;"

# Dump and restore
sqlite3 orchestrix.db ".dump" > backup.sql
rm orchestrix.db
sqlite3 orchestrix.db < backup.sql

# Or start fresh
mv ~/.orchestrix/orchestrix.db ~/.orchestrix/orchestrix.db.corrupted
```

## Frontend Issues

### Hot Reload Not Working

**Symptom**: Changes not reflected in UI

**Solutions**:

```bash
# Restart dev server
Ctrl+C
bun tauri dev

# Clear Vite cache
rm -rf node_modules/.vite
bun tauri dev

# Check for TypeScript errors
bun run tsc --noEmit
```

### Component Not Rendering

**Symptom**: Blank area where component should be

**Solutions**:

1. Check browser console for errors (F12)
2. Verify component is imported correctly
3. Check for missing props
4. Add ErrorBoundary to catch errors:
   ```tsx
   <ErrorBoundary>
     <MyComponent {...props} />
   </ErrorBoundary>
   ```

### Events Not Updating UI

**Symptom**: Backend events not reflected in frontend

**Solutions**:

1. Check event channel is connected:
   ```typescript
   // In browser console
   window.__TAURI__.event.listen('orchestrix://events', console.log)
   ```

2. Verify store is processing events:
   ```typescript
   // Check store state
   useAppStore.getState().events
   ```

3. Restart the application

### TypeScript Errors

**Error**: Type errors in IDE or build

**Solutions**:

```bash
# Run type checker
bun run tsc

# Fix specific errors following compiler output
# Common issues:
# - Missing type imports
# - Incorrect prop types
# - Type mismatches between Rust/TypeScript
```

## UX and Transparency Issues

### Timeline Feels Too Noisy

**Symptom**: Users see too many low-value rows and lose the main execution story.

**Solutions**:

1. Group event rows by phase or step in the conversation timeline
2. Keep summary rows collapsed by default and expand only on demand
3. Prioritize warnings/failures ahead of successful routine events
4. Ensure duplicate status indicators are removed in favor of one authoritative timeline

### Missing AI Activity Visibility

**Symptom**: The UI appears stuck or users cannot tell what the agent is doing.

**Solutions**:

1. Verify immediate feedback events are being emitted (`agent.deciding`, `agent.tool_calls_preparing`)
2. Confirm `tool.call_started` and `tool.call_finished` are present in incoming batches
3. Check event-to-conversation mapping in runtime event buffer
4. Confirm the selected task context matches incoming `task_id`/`run_id`

### Long Runs Cause UI Slowness

**Symptom**: Timeline scrolling and updates become sluggish on large runs.

**Solutions**:

1. Add list virtualization/windowing for long timelines
2. Process batches incrementally instead of reprocessing full history each tick
3. Use narrow selectors (`useShallow`) to avoid global re-renders
4. Reduce expensive formatting work in render paths

## Development Issues

### Changes Not Persisting

**Symptom**: Code changes revert or don't save

**Solutions**:

1. Check file permissions
2. Verify IDE is not reverting changes
3. Check for format-on-save conflicts
4. Ensure you're editing the right file:
   ```bash
   # Check which files are being watched
   find . -name "*.rs" -o -name "*.ts" -o -name "*.tsx" | head -20
   ```

### Git Hooks Failing

**Error**: Pre-commit hooks prevent commits

**Solutions**:

```bash
# Run formatter manually
cargo fmt
cd src-tauri && cargo clippy -- -W clippy::all

# Skip hooks temporarily (not recommended)
git commit --no-verify -m "message"

# Fix the underlying issue first
```

### IDE Not Recognizing Types

**Symptom**: TypeScript errors in IDE but not in build

**Solutions**:

1. Restart TypeScript server in IDE
2. Check `tsconfig.json` paths are correct
3. Regenerate types if needed:
   ```bash
   bun install
   ```
4. Check IDE is using workspace TypeScript version

### Slow Development Builds

**Symptom**: Takes long time to rebuild

**Solutions**:

```bash
# Use release mode for testing (faster runtime)
bun tauri dev --release

# Or optimize debug builds
# Add to src-tauri/Cargo.toml:
# [profile.dev]
# opt-level = 1

# Disable unnecessary features
# Edit tauri.conf.json to disable unused plugins
```

## Test Failures

### Rust Tests Failing

**Solutions**:

```bash
# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture

# Run ignored tests (integration tests)
cargo test -- --ignored

# Check for environment variables needed
export MINIMAX_API_KEY=test-key
cargo test
```

### Frontend Tests (When Available)

```bash
# Run Vitest tests
bun test

# Run with UI
bun test --ui

# Run specific file
bun test MyComponent.test.tsx
```

## Performance Issues

### High Memory Usage

**Solutions**:

```bash
# Limit event batch size
# Edit src-tauri/src/bus/batcher.rs
# const MAX_BATCH_SIZE: usize = 25;  // Reduce from 50

# Clear old events
sqlite3 ~/.orchestrix/orchestrix.db "DELETE FROM events WHERE created_at < datetime('now', '-7 days');"

# Use production build
bun tauri build
```

### Slow UI Rendering

**Solutions**:

1. Check for unnecessary re-renders (React DevTools)
2. Use `useShallow` for store selectors
3. Virtualize long lists
4. Reduce event frequency in backend

## Getting Help

### Before Asking

1. Check this troubleshooting guide
2. Review logs: `RUST_LOG=debug bun tauri dev`
3. Check browser console for frontend errors
4. Verify your setup matches SETUP.md

### Gathering Information

When reporting issues, include:

```bash
# System information
rustc --version
bun --version
node --version
uname -a  # or systeminfo on Windows

# Application version
cat package.json | grep version
cat src-tauri/Cargo.toml | grep version

# Recent logs
RUST_LOG=orchestrix=debug bun tauri dev 2>&1 | tail -100
```

### Where to Get Help

- **GitHub Issues**: Report bugs and feature requests
- **Documentation**: Check README.md, UX_PRINCIPLES.md, ARCHITECTURE.md, CODING_STANDARDS.md
- **Examples**: See examples/ directory
- **Logs**: Enable debug logging for detailed information

### Debug Mode

Enable maximum logging for troubleshooting:

```bash
# All debug logging
RUST_LOG=trace bun tauri dev

# Specific module logging
RUST_LOG=orchestrix::runtime=debug bun tauri dev
RUST_LOG=orchestrix::db=trace bun tauri dev

# Frontend debug
# Open DevTools with F12 in development mode
```

## Quick Fixes Checklist

When something breaks, try in order:

1. [ ] Restart the application
2. [ ] Clear build cache: `rm -rf src-tauri/target node_modules/.vite`
3. [ ] Reinstall dependencies: `rm -rf node_modules && bun install`
4. [ ] Update Rust: `rustup update`
5. [ ] Check environment variables
6. [ ] Review recent code changes
7. [ ] Reset database (if safe): `rm ~/.orchestrix/orchestrix.db`
8. [ ] Clean git state: `git clean -fdx` (WARNING: removes untracked files)

## Known Issues

### Platform-Specific

**macOS**:
- Gatekeeper may block unsigned builds
- Solution: Allow in System Preferences → Security

**Windows**:
- Long path issues with node_modules
- Solution: Enable long path support in Windows

**Linux**:
- Missing system libraries for Tauri
- Solution: Install dependencies per SETUP.md

### Version-Specific

Check the GitHub repository for:
- Known issues with specific versions
- Migration guides between versions
- Breaking changes in dependencies
