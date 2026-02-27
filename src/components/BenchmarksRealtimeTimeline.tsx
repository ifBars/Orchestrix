import type { BenchmarkRealtimeEvent } from "@/types";

type BenchmarksRealtimeTimelineProps = {
  events: BenchmarkRealtimeEvent[];
};

function describeEvent(event: BenchmarkRealtimeEvent): string {
  switch (event.kind) {
    case "run_started":
      return `Run started (${event.providers.length} providers, ${event.scenario_count} scenarios)`;
    case "provider_started":
      return `${event.provider} started (${event.model ?? "default model"})`;
    case "scenario_started":
      return `${event.provider} -> ${event.scenario_id} (iteration ${event.iteration})`;
    case "prompt_completed":
      return `Day ${event.day_index} prompt ${event.prompt_index}: ${event.action_kind} (${event.tool_calls} tool calls)`;
    case "warning":
      return `Warning day ${event.day_index} prompt ${event.prompt_index}: ${event.message}`;
    case "day_completed":
      return `Day ${event.day_index} complete | cash ${event.ending_cash.toFixed(0)} | profit ${event.profit_to_date.toFixed(0)}`;
    case "scenario_completed":
      return `${event.scenario_id} complete | score ${event.final_score.toFixed(3)} | profit ${event.raw_profit.toFixed(0)}`;
    case "run_completed":
      return "Run completed";
  }
}

export function BenchmarksRealtimeTimeline({ events }: BenchmarksRealtimeTimelineProps) {
  return (
    <section className="rounded-xl border border-border/80 bg-card/90 p-4 elevation-1 backdrop-blur-sm">
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-sm font-semibold tracking-tight text-foreground">Realtime Timeline</h3>
        <span className="text-xs text-muted-foreground">{events.length} events</span>
      </div>
      {events.length === 0 ? (
        <p className="text-xs text-muted-foreground">No events yet.</p>
      ) : (
        <div className="max-h-64 space-y-1 overflow-y-auto rounded-md border border-border/70 bg-background/70 p-2">
          {events.slice().reverse().map((event, idx) => {
            const warning = event.kind === "warning";
            return (
              <div
                key={`${event.kind}-${event.run_id}-${idx}`}
                className={`rounded border px-2 py-1 text-xs ${warning ? "border-warning/40 bg-warning/10 text-warning" : "border-border/60 text-muted-foreground"}`}
              >
                {describeEvent(event)}
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}
