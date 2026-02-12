import * as React from "react";
import { cn } from "@/lib/utils";

export function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  return (
    <textarea
      data-slot="textarea"
      className={cn(
        "min-h-24 w-full rounded-md border border-input/80 bg-background/85 px-3 py-2 text-sm font-normal text-foreground outline-none transition-colors placeholder:text-muted-foreground/80 focus-visible:border-ring/70 focus-visible:ring-2 focus-visible:ring-ring/40",
        className
      )}
      {...props}
    />
  );
}
