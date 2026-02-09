export function UserMessageItem({ content }: { content: string }) {
  return (
    <div className="flex gap-3">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary/15 text-primary">
        <span className="text-xs font-semibold">You</span>
      </div>
      <div className="min-w-0 flex-1 pt-1">
        <p className="text-sm leading-relaxed text-foreground">{content}</p>
      </div>
    </div>
  );
}
