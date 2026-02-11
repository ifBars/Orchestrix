# Examples

This directory contains example implementations and guides for common tasks in Orchestrix.

All examples should preserve the core product contract: human-in-the-loop review, full AI transparency, and condensed non-cluttered UI rendering.

## Available Examples

### Backend (Rust)

- **[adding-a-command](./adding-a-command.md)** - How to add a new Tauri command
- **[adding-a-tool](./adding-a-tool.md)** - How to implement and register a new tool
- **[database-queries](./database-queries.md)** - Working with the database layer
- **[event-system](./event-system.md)** - Understanding and using the event system

### Frontend (TypeScript/React)

- **[adding-a-component](./adding-a-component.md)** - Creating new React components
- **[working-with-stores](./working-with-stores.md)** - State management patterns
- **[event-consumption](./event-consumption.md)** - Consuming backend events in React

### Skills System

- **[creating-a-skill](./creating-a-skill.md)** - Building custom skills for agents
- **[skill-manifest](./skill-manifest.md)** - Understanding skill configuration

## Quick Reference

### Common Tasks

#### Add a new Tauri command

```rust
// src-tauri/src/commands/my_feature.rs
#[tauri::command]
pub async fn my_command(
    state: tauri::State<'_, AppState>,
    param: String,
) -> Result<MyResult, AppError> {
    // Implementation
}
```

#### Add a new React component

```tsx
// src/components/MyFeature/MyComponent.tsx
type MyComponentProps = {
  data: MyDataType;
};

export function MyComponent({ data }: MyComponentProps) {
  return <div>{data.content}</div>;
}
```

#### Listen for backend events

```tsx
import { listen } from "@tauri-apps/api/event";

useEffect(() => {
  const unlisten = listen<BusEvent[]>("orchestrix://events", (event) => {
    // Handle events
  });
  
  return () => { unlisten.then(f => f()); };
}, []);
```

## Contributing Examples

To add a new example:

1. Create a new `.md` file in this directory
2. Follow the existing format with clear explanations
3. Include complete, runnable code samples
4. Add the example to the list above
5. Update the Quick Reference section if applicable
