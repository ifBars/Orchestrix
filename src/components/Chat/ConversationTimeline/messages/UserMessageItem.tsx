export function UserMessageItem({ content }: { content: string }) {
  return (
    <div className="flex gap-3">
      <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-primary/25 bg-primary/12 text-primary">
        <span className="text-xs font-semibold">You</span>
      </div>
      <div className="min-w-0 flex-1 rounded-xl border border-border/70 bg-background/55 px-3 py-2.5">
        <p className="text-sm leading-relaxed text-foreground">{content}</p>
      </div>
    </div>
  );
}
