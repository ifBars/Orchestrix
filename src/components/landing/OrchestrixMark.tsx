import orchestrixIcon from "../../../src-tauri/icons/icon.png";
import { cn } from "@/lib/utils";

type OrchestrixMarkProps = {
  className?: string;
};

export function OrchestrixMark({ className }: OrchestrixMarkProps) {
  return (
    <img
      src={orchestrixIcon}
      alt=""
      aria-hidden="true"
      draggable={false}
      className={cn("h-7 w-7 select-none", className)}
    />
  );
}
