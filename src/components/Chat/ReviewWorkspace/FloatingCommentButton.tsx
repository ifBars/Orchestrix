import { MessageSquarePlus } from "lucide-react";
import type { HoverState } from "./useCommentHover";

type FloatingCommentButtonProps = {
  hoverState: HoverState;
  buttonRef: React.RefObject<HTMLButtonElement | null>;
  draftLine: number | null;
  onMouseEnter: () => void;
  onMouseLeave: () => void;
  onClick: () => void;
  getButtonStyle: () => React.CSSProperties;
};

export function FloatingCommentButton({
  hoverState,
  buttonRef,
  draftLine,
  onMouseEnter,
  onMouseLeave,
  onClick,
  getButtonStyle,
}: FloatingCommentButtonProps) {
  if (!hoverState || draftLine != null) return null;

  return (
    <button
      ref={buttonRef}
      type="button"
      onClick={onClick}
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
      className="absolute z-30 flex h-6 w-6 items-center justify-center rounded border border-primary/40 bg-primary/15 text-primary shadow-sm backdrop-blur-sm transition-all duration-150 hover:scale-110 hover:bg-primary/25"
      style={getButtonStyle()}
      title={`Comment on block ${hoverState.line}`}
    >
      <MessageSquarePlus size={12} />
    </button>
  );
}
