# Consuming Backend Events

This guide explains how to receive and process events from the Rust backend in the React frontend.

## Overview

Orchestrix uses an **event-driven architecture** where the backend emits events that the frontend consumes. This enables real-time updates for task progress, tool calls, and agent messages.

## Event Flow

```
Backend                    Tauri IPC                  Frontend
--------                   -----------                  --------
EventBus.emit()    ---->   orchestrix://    ---->   listen()
  (Rust)                     (channel)                   (React)
                                │
                                ▼
                         EventBatcher
                         (100ms batch)
                                │
                                ▼
                         runtimeEventBuffer
                         (transform events)
                                │
                                ▼
                         appStore.processEvents()
                                │
                                ▼
                         React Re-render
```

## Setting Up Event Listening

### In the Store (appStore.ts)

Events are subscribed to during bootstrap:

```typescript
// src/stores/appStore.ts

bootstrap: async () => {
  // ... other bootstrap code ...
  
  // Subscribe to events
  const unlisten = await listen<BusEvent[]>(
    "orchestrix://events",
    (event) => {
      const events = event.payload;
      
      // Transform raw events to conversation items
      const items = runtimeEventBuffer.processEvents(events);
      
      // Update store with new events
      set((state) => ({
        events: [...state.events, ...events],
        conversationItems: [...state.conversationItems, ...items],
      }));
      
      // Handle specific event types
      for (const evt of events) {
        handleSpecialEvent(evt);
      }
    }
  );
  
  // Store unlisten function for cleanup
  set({ eventUnlisten: unlisten });
},

shutdown: () => {
  const { eventUnlisten } = get();
  if (eventUnlisten) {
    eventUnlisten();
  }
},
```

### In a Component

For component-specific event handling:

```tsx
// src/components/TaskMonitor/TaskMonitor.tsx

import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useAppStore } from "@/store";
import type { BusEvent } from "@/types";

export function TaskMonitor({ taskId }: { taskId: string }) {
  const selectedTaskId = useAppStore((state) => state.selectedTaskId);
  
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    
    const setupListener = async () => {
      unlisten = await listen<BusEvent[]>(
        "orchestrix://events",
        (event) => {
          const events = event.payload;
          
          // Filter events for this task
          const taskEvents = events.filter(
            (e) => e.payload.task_id === taskId
          );
          
          // Handle specific events
          for (const evt of taskEvents) {
            console.log(`Event: ${evt.event_type}`);
            
            switch (evt.event_type) {
              case "agent.step_started":
                handleStepStarted(evt);
                break;
              case "tool.call_finished":
                handleToolFinished(evt);
                break;
              case "agent.subagent_completed":
                handleSubAgentCompleted(evt);
                break;
            }
          }
        }
      );
    };
    
    setupListener();
    
    // Cleanup on unmount
    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [taskId]);
  
  return <div>Monitoring task {taskId}</div>;
}
```

## Event Types

### BusEvent Structure

```typescript
interface BusEvent {
  id: string;              // UUID
  run_id: string | null;   // Associated run
  seq: number;             // Monotonic sequence (for ordering)
  category: string;        // Event namespace
  event_type: string;      // Specific event
  payload: Record<string, unknown>;  // Event data
  created_at: string;      // RFC 3339 timestamp
}
```

### Common Event Categories

#### Task Events

```typescript
// task.created
{
  category: "task",
  event_type: "task.created",
  payload: {
    task_id: "uuid",
    prompt: "Create a component",
  }
}

// task.status_changed
{
  category: "task",
  event_type: "task.status_changed",
  payload: {
    task_id: "uuid",
    old_status: "pending",
    new_status: "executing",
  }
}
```

#### Agent Events

```typescript
// agent.planning_started
{
  category: "agent",
  event_type: "agent.planning_started",
  payload: {
    task_id: "uuid",
    run_id: "uuid",
  }
}

// agent.plan_ready
{
  category: "agent",
  event_type: "agent.plan_ready",
  payload: {
    task_id: "uuid",
    run_id: "uuid",
    plan_json: "{...}",
  }
}

// agent.step_started
{
  category: "agent",
  event_type: "agent.step_started",
  payload: {
    task_id: "uuid",
    run_id: "uuid",
    step_idx: 0,
    step_title: "Analyze requirements",
  }
}

// agent.subagent_completed
{
  category: "agent",
  event_type: "agent.subagent_completed",
  payload: {
    task_id: "uuid",
    run_id: "uuid",
    sub_agent_id: "uuid",
    success: true,
  }
}
```

#### Tool Events

```typescript
// tool.call_started
{
  category: "tool",
  event_type: "tool.call_started",
  payload: {
    task_id: "uuid",
    run_id: "uuid",
    tool_call_id: "uuid",
    tool_name: "read_file",
    input: { path: "src/main.rs" },
  }
}

// tool.call_finished
{
  category: "tool",
  event_type: "tool.call_finished",
  payload: {
    task_id: "uuid",
    run_id: "uuid",
    tool_call_id: "uuid",
    success: true,
    output: "file content...",
  }
}
```

## Transforming Events

The `runtimeEventBuffer` transforms raw events into conversation items:

```typescript
// src/runtime/eventBuffer.ts

export function processEvents(events: BusEvent[]): ConversationItem[] {
  const items: ConversationItem[] = [];
  
  for (const event of events) {
    const item = transformEvent(event);
    if (item) {
      items.push(item);
    }
  }
  
  return items;
}

function transformEvent(event: BusEvent): ConversationItem | null {
  switch (event.event_type) {
    case "task.created":
      return {
        id: event.id,
        type: "userMessage",
        timestamp: event.created_at,
        seq: event.seq,
        content: event.payload.prompt,
      };
      
    case "agent.plan_message":
      return {
        id: event.id,
        type: "agentMessage",
        timestamp: event.created_at,
        seq: event.seq,
        content: event.payload.message,
      };
      
    case "agent.step_started":
      return {
        id: event.id,
        type: "planStep",
        timestamp: event.created_at,
        seq: event.seq,
        stepIndex: event.payload.step_idx,
        stepTitle: event.payload.step_title,
        stepDescription: event.payload.description,
      };
      
    case "tool.call_started":
      return {
        id: event.id,
        type: "toolCall",
        timestamp: event.created_at,
        seq: event.seq,
        toolName: event.payload.tool_name,
        toolArgs: event.payload.input,
        toolStatus: "running",
      };
      
    default:
      return null;
  }
}
```

## Handling Event Batches

Events arrive in batches (50 events max, 100ms flush):

```typescript
// Process batch efficiently
const handleEventBatch = (events: BusEvent[]) => {
  // Group by type for efficient processing
  const byType = events.reduce((acc, event) => {
    if (!acc[event.event_type]) {
      acc[event.event_type] = [];
    }
    acc[event.event_type].push(event);
    return acc;
  }, {} as Record<string, BusEvent[]>);
  
  // Process each type
  if (byType["tool.call_finished"]) {
    updateToolResults(byType["tool.call_finished"]);
  }
  
  if (byType["agent.step_completed"]) {
    updateStepStatus(byType["agent.step_completed"]);
  }
  
  // ... etc
};
```

## Event Ordering

Use the `seq` field to maintain order:

```typescript
// Sort events by sequence number
const sortedEvents = [...events].sort((a, b) => a.seq - b.seq);

// Process in order
for (const event of sortedEvents) {
  processEvent(event);
}
```

## Error Handling

Handle malformed events gracefully:

```typescript
const handleEvent = (event: BusEvent) => {
  try {
    switch (event.event_type) {
      case "tool.call_finished": {
        // Validate payload
        if (!event.payload.tool_call_id) {
          console.warn("Missing tool_call_id in event", event);
          return;
        }
        
        processToolResult(event.payload);
        break;
      }
      // ... other cases
    }
  } catch (error) {
    console.error("Failed to process event:", event, error);
    // Don't throw - continue processing other events
  }
};
```

## Best Practices

### 1. Filter Early

Filter events at the listener level to reduce processing:

```typescript
// Good - filter before processing
const taskEvents = events.filter(e => e.payload.task_id === taskId);

// Bad - process then filter
for (const event of events) {
  if (event.payload.task_id === taskId) {
    // process
  }
}
```

### 2. Debounce High-Frequency Events

```typescript
import { useMemo } from "react";
import { debounce } from "lodash";

function useDebouncedEventHandler() {
  return useMemo(
    () =>
      debounce((events: BusEvent[]) => {
        // Expensive processing
      }, 100),
    []
  );
}
```

### 3. Update State Immutably

```typescript
// Good - immutable update
set((state) => ({
  events: [...state.events, ...newEvents],
}));

// Bad - mutating state
state.events.push(...newEvents);
```

### 4. Clean Up Listeners

Always clean up event listeners to prevent memory leaks:

```typescript
useEffect(() => {
  let unlisten: UnlistenFn;
  
  listen(...).then((fn) => {
    unlisten = fn;
  });
  
  return () => {
    unlisten?.();
  };
}, []);
```

## Testing Event Handling

```typescript
// Test event processing
const testEvent: BusEvent = {
  id: "test-uuid",
  run_id: "run-uuid",
  seq: 1,
  category: "tool",
  event_type: "tool.call_finished",
  payload: {
    task_id: "task-uuid",
    tool_call_id: "call-uuid",
    success: true,
    output: "result",
  },
  created_at: new Date().toISOString(),
};

// Process in test
runtimeEventBuffer.processEvents([testEvent]);

// Assert expected state
expect(store.getState().conversationItems).toHaveLength(1);
```

## See Also

- [ARCHITECTURE.md](../ARCHITECTURE.md) - Event System section
- [CODING_STANDARDS.md](../CODING_STANDARDS.md) - Event Naming
- [Tauri Event API](https://tauri.app/v1/api/js/event/)
