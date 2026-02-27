import { CheckCircle2, Map, Rocket, Sparkles } from "lucide-react";
import { useShallow } from "zustand/shallow";
import { useAppStore } from "@/stores/appStore";
import { cn } from "@/lib/utils";

const WORKFLOW_MODES = [
  {
    id: "plan",
    label: "Plan",
    icon: Map,
  },
  {
    id: "build",
    label: "Build",
    icon: Rocket,
  },
] as const;

export function EmptyState() {
  const [workflowMode, setWorkflowMode] = useAppStore(
    useShallow((state) => [state.workflowMode, state.setWorkflowMode])
  );

  return (
    <div className="flex h-full items-center justify-center p-4">
      <div className="w-full max-w-md text-center">
        <div className="mb-6 inline-flex h-12 w-12 items-center justify-center rounded-xl border border-border/60 bg-card shadow-sm">
          <Sparkles className="h-6 w-6 text-primary" />
        </div>

        <h1 className="text-xl font-semibold tracking-tight text-foreground">
          Start an orchestrated run
        </h1>
        <p className="mx-auto mt-2 max-w-sm text-sm text-muted-foreground">
          Describe your goal. Orchestrix plans first, executes with full tool visibility, and keeps review in the loop.
        </p>

        <div className="mt-6 inline-flex items-center rounded-lg border border-border/70 bg-card/60 p-1">
          {WORKFLOW_MODES.map((workflow) => {
            const isSelected = workflowMode === workflow.id;
            const Icon = workflow.icon;
            return (
              <button
                key={workflow.id}
                type="button"
                onClick={() => setWorkflowMode(workflow.id)}
                className={cn(
                  "flex items-center gap-2 rounded-md px-4 py-2 text-sm font-medium transition-all",
                  isSelected
                    ? "bg-primary/12 text-foreground shadow-sm"
                    : "text-muted-foreground hover:text-foreground"
                )}
              >
                <Icon className="h-4 w-4" />
                {workflow.label}
                {isSelected && <CheckCircle2 className="ml-1 h-3.5 w-3.5 text-primary" />}
              </button>
            );
          })}
        </div>

        <p className="mt-4 text-xs text-muted-foreground/70">
          {workflowMode === "plan"
            ? "Explores workspace, creates plan, waits for approval"
            : "Executes directly with full tool visibility"}
        </p>
      </div>
    </div>
  );
}
