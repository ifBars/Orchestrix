import type { BusEvent } from "@/types";

type DebugEventsProps = {
  rawEvents: BusEvent[];
};

export function DebugEvents({ rawEvents }: DebugEventsProps) {
  if (rawEvents.length === 0) return null;

  const recent = rawEvents.slice(-80).reverse();
  return (
    <details className="rounded-xl border border-border/70 bg-muted/10 p-3">
      <summary className="cursor-pointer text-xs font-medium text-muted-foreground">
        Debug Timeline ({rawEvents.length} events)
      </summary>
      <div className="mt-3 max-h-72 space-y-2 overflow-auto">
        {recent.map((event) => (
          <div key={event.id} className="rounded-md border border-border/50 bg-background/70 p-2">
            <div className="mb-1 flex items-center justify-between gap-2 text-[11px] text-muted-foreground">
              <span className="truncate">{event.event_type}</span>
              <span>{new Date(event.created_at).toLocaleTimeString()}</span>
            </div>
            <pre className="overflow-auto text-[11px] text-foreground/80">
              <code>{JSON.stringify(event.payload, null, 2)}</code>
            </pre>
          </div>
        ))}
      </div>
    </details>
  );
}
