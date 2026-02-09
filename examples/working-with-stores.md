# Working with Stores

This guide explains how to use Zustand stores for state management in Orchestrix.

## Overview

Orchestrix uses Zustand for state management with two main stores:
- **appStore** - Main application state (tasks, events, settings)
- **streamStore** - High-frequency reactive counters

## appStore

The main store is defined in `src/stores/appStore.ts`.

### Store Structure

```typescript
// Simplified view of appStore structure
type AppStoreState = {
  // State
  tasks: TaskRow[];
  selectedTaskId: string | null;
  events: EventRow[];
  artifacts: ArtifactRow[];
  conversationItems: ConversationItem[];
  isBootstrapped: boolean;
  workspaceRoot: string;
  providerConfigs: ProviderConfigView[];
  
  // Actions
  bootstrap: () => Promise<void>;
  createTask: (prompt: string, options?: CreateTaskOptions) => Promise<void>;
  startTask: (taskId: string, mode?: "plan" | "build") => Promise<void>;
  deleteTask: (taskId: string) => Promise<void>;
  selectTask: (taskId: string | null) => void;
  shutdown: () => void;
};
```

### Basic Usage

```tsx
import { useAppStore } from "@/store";

function MyComponent() {
  // Single value selector (no re-render when other values change)
  const selectedTaskId = useAppStore((state) => state.selectedTaskId);
  
  // Action selector
  const createTask = useAppStore((state) => state.createTask);
  
  return (
    <button onClick={() => createTask("My task")}>
      Create Task
    </button>
  );
}
```

### Selecting Multiple Values

Use `useShallow` to prevent re-renders when other state changes:

```tsx
import { useAppStore } from "@/store";
import { useShallow } from "zustand/shallow";

function TaskList() {
  // Good - only re-renders when these specific values change
  const [tasks, selectedTaskId, selectTask] = useAppStore(
    useShallow((state) => [
      state.tasks,
      state.selectedTaskId,
      state.selectTask,
    ])
  );
  
  return (
    <ul>
      {tasks.map((task) => (
        <li
          key={task.id}
          onClick={() => selectTask(task.id)}
          className={task.id === selectedTaskId ? "selected" : ""}
        >
          {task.prompt}
        </li>
      ))}
    </ul>
  );
}
```

### Avoid This Pattern

```tsx
// Bad - re-renders on every state change
const store = useAppStore();
const { tasks, createTask } = store;
```

## Adding State to the Store

### Step 1: Define the State Type

```typescript
// In src/stores/appStore.ts

type AppStoreState = {
  // ... existing state ...
  
  // Add new state
  myFeature: MyFeatureState;
};

type MyFeatureState = {
  data: MyDataType[];
  isLoading: boolean;
  error: string | null;
};
```

### Step 2: Add Initial State

```typescript
export const useAppStore = create<AppStoreState>((set, get) => ({
  // ... existing initial state ...
  
  // Add initial state
  myFeature: {
    data: [],
    isLoading: false,
    error: null,
  },
  
  // ... rest of store ...
}));
```

### Step 3: Add Actions

```typescript
export const useAppStore = create<AppStoreState>((set, get) => ({
  // ... existing state and actions ...
  
  // Add new actions
  loadMyFeature: async () => {
    set((state) => ({
      myFeature: { ...state.myFeature, isLoading: true, error: null },
    }));
    
    try {
      const data = await invoke<MyDataType[]>("get_my_feature_data");
      
      set((state) => ({
        myFeature: { ...state.myFeature, data, isLoading: false },
      }));
    } catch (error) {
      set((state) => ({
        myFeature: {
          ...state.myFeature,
          isLoading: false,
          error: error instanceof Error ? error.message : "Unknown error",
        },
      }));
    }
  },
  
  clearMyFeature: () => {
    set((state) => ({
      myFeature: { data: [], isLoading: false, error: null },
    }));
  },
}));
```

## Derived Selectors

Create custom hooks for derived state:

```typescript
// In src/stores/appStore.ts

// Derived selector hook
export const useSelectedTask = () =>
  useAppStore((state) =>
    state.tasks.find((t) => t.id === state.selectedTaskId)
  );

// Usage in component
function TaskDetail() {
  const selectedTask = useSelectedTask();
  
  if (!selectedTask) return <div>No task selected</div>;
  
  return <div>{selectedTask.prompt}</div>;
}
```

## Working with Events

The store automatically processes events from the backend:

```typescript
// Event processing happens in the store's bootstrap
processEvents: (events: BusEvent[]) => {
  for (const event of events) {
    switch (event.event_type) {
      case "task.created":
        // Update tasks list
        break;
      case "task.status_changed":
        // Update task status
        break;
      // ... handle other events
    }
  }
},
```

## streamStore (High-Frequency Updates)

For values that update rapidly without triggering full re-renders:

```typescript
// src/stores/streamStore.ts
import { create } from "zustand";

type StreamTickStore = {
  planTickByTask: Record<string, number>;
  eventCount: number;
  incrementPlanTick: (taskId: string) => void;
};

export const useStreamTickStore = create<StreamTickStore>((set) => ({
  planTickByTask: {},
  eventCount: 0,
  
  incrementPlanTick: (taskId) =>
    set((state) => ({
      planTickByTask: {
        ...state.planTickByTask,
        [taskId]: (state.planTickByTask[taskId] || 0) + 1,
      },
    })),
}));
```

## Best Practices

### 1. Keep Stores Focused

Don't put everything in appStore. Create separate stores for distinct features.

### 2. Use Selectors Properly

```tsx
// Good - specific selector
const taskCount = useAppStore((state) => state.tasks.length);

// Bad - selects entire array
const tasks = useAppStore((state) => state.tasks);
```

### 3. Handle Async Operations

```typescript
// Always handle errors in async actions
asyncAction: async () => {
  set({ isLoading: true });
  try {
    const result = await apiCall();
    set({ data: result, isLoading: false });
  } catch (error) {
    set({ error: error.message, isLoading: false });
  }
},
```

### 4. Use TypeScript Strictly

```typescript
// Define types clearly
type AppStoreState = {
  tasks: TaskRow[];
  selectedTaskId: string | null;  // null is explicit
};

// Type the store
create<AppStoreState>((set) => ({ ... }));
```

### 5. Persist State When Needed

```typescript
import { persist } from "zustand/middleware";

const usePersistentStore = create(
  persist<StoreState>(
    (set) => ({
      // state
    }),
    {
      name: "my-storage-key",
    }
  )
);
```

## Common Patterns

### Optimistic Updates

```typescript
updateTask: async (taskId, updates) => {
  const previousTasks = get().tasks;
  
  // Optimistically update
  set((state) => ({
    tasks: state.tasks.map((t) =>
      t.id === taskId ? { ...t, ...updates } : t
    ),
  }));
  
  try {
    await invoke("update_task", { taskId, updates });
  } catch (error) {
    // Rollback on error
    set({ tasks: previousTasks });
    throw error;
  }
},
```

### Computed Values

```typescript
// Use selector for computed values
export const useTaskStats = (taskId: string) =>
  useAppStore((state) => {
    const task = state.tasks.find((t) => t.id === taskId);
    const runs = state.runs.filter((r) => r.task_id === taskId);
    
    return {
      runCount: runs.length,
      isActive: task?.status === "executing",
      duration: calculateDuration(runs),
    };
  });
```

## Debugging

Enable Zustand devtools:

```typescript
import { devtools } from "zustand/middleware";

const useAppStore = create(
  devtools<AppStoreState>(
    (set, get) => ({
      // store definition
    }),
    { name: "AppStore" }
  )
);
```

Access the store in browser console:
```javascript
// After installing Redux DevTools extension
const state = useAppStore.getState();
console.log(state.tasks);
```

## See Also

- [Zustand Documentation](https://github.com/pmndrs/zustand)
- [CODING_STANDARDS.md](../CODING_STANDARDS.md) - State Management section
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Component Overview
