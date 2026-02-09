import { useEffect, useState } from "react";
import { ChevronDown, ChevronRight, ListChecks } from "lucide-react";
import { runtimeEventBuffer } from "@/runtime/eventBuffer";
import { todoStatusClass } from "./utils";

type AgentTodoPanelProps = {
  agentTodos: ReturnType<typeof runtimeEventBuffer.getAgentTodos>;
  isWorking: boolean;
};

export function AgentTodoPanel({ agentTodos, isWorking }: AgentTodoPanelProps) {
  const [expanded, setExpanded] = useState(isWorking);

  useEffect(() => {
    setExpanded(isWorking);
  }, [isWorking]);

  const total = agentTodos.reduce((sum, list) => sum + list.todos.length, 0);
  const completed = agentTodos.reduce(
    (sum, list) => sum + list.todos.filter((todo) => todo.status === "completed").length,
    0
  );
  const inProgress = agentTodos.reduce(
    (sum, list) => sum + list.todos.filter((todo) => todo.status === "in_progress").length,
    0
  );

  return (
    <div className="rounded-xl border border-border/70 bg-background/90 p-3 shadow-sm backdrop-blur supports-[backdrop-filter]:bg-background/80">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-2 text-left"
      >
        <ListChecks size={14} className="text-muted-foreground" />
        <span className="text-xs font-medium text-foreground">Todo lists ({total})</span>
        <span className="ml-auto text-[11px] text-muted-foreground">
          {completed} completed{inProgress > 0 ? `, ${inProgress} in progress` : ""}
        </span>
        {expanded ? (
          <ChevronDown size={12} className="text-muted-foreground" />
        ) : (
          <ChevronRight size={12} className="text-muted-foreground" />
        )}
      </button>

      {expanded && (
        <div className="mt-3 space-y-2">
          {agentTodos.map((list) => (
            <div
              key={list.agentId}
              className="rounded-lg border border-border/60 bg-card/40 p-2.5"
            >
              <div className="mb-2 flex items-center justify-between gap-2">
                <span className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
                  {list.agentId === "main" ? "Main agent" : `Sub-agent ${list.agentId.slice(0, 8)}`}
                </span>
                <span className="text-[11px] text-muted-foreground">{list.todos.length} items</span>
              </div>

              <div className="space-y-1.5">
                {list.todos.map((todo) => (
                  <div key={`${list.agentId}-${todo.id}`} className="flex items-start gap-2">
                    <span className={todoStatusClass(todo.status)}>{todo.status.replace(/_/g, " ")}</span>
                    <p className="min-w-0 flex-1 text-xs text-foreground">{todo.content}</p>
                    {todo.priority && (
                      <span className="rounded-full border border-border/60 bg-background/70 px-1.5 py-0.5 text-[10px] text-muted-foreground">
                        {todo.priority}
                      </span>
                    )}
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
