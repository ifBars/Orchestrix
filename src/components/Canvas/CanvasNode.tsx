/**
 * CanvasNode.tsx
 *
 * Custom React Flow node with:
 * - Inline label/description editing on double-click
 * - Delete button visible on hover/select
 * - Connectable handles (source + target)
 * - Kind badge
 *
 * Uses inline styles throughout to avoid Tailwind v4 / React Flow CSS conflicts.
 * Callbacks (onEdit, onDelete) are passed via node.data by ArchitectureCanvas.
 */

import { memo, useCallback, useEffect, useRef, useState } from "react";
import { Handle, Position, type NodeProps } from "reactflow";

export type CanvasNodeData = {
  label: string;
  kind?: string;
  description?: string;
  positionSet?: boolean;
  /** Injected by ArchitectureCanvas — called when the user commits an edit. */
  onEdit?: (id: string, label: string, kind: string, description: string) => void;
  /** Injected by ArchitectureCanvas — called when the user deletes the node. */
  onDelete?: (id: string) => void;
};

// ── Inline editor ─────────────────────────────────────────────────────────────

type EditPopoverProps = {
  id: string;
  label: string;
  kind: string;
  description: string;
  onSave: (label: string, kind: string, description: string) => void;
  onClose: () => void;
};

function EditPopover({ id: _id, label, kind, description, onSave, onClose }: EditPopoverProps) {
  const [l, setL] = useState(label);
  const [k, setK] = useState(kind);
  const [d, setD] = useState(description);
  const labelRef = useRef<HTMLInputElement>(null);

  useEffect(() => { labelRef.current?.focus(); }, []);

  const commit = useCallback(() => {
    if (l.trim()) onSave(l.trim(), k.trim(), d.trim());
    onClose();
  }, [l, k, d, onSave, onClose]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); commit(); }
    if (e.key === "Escape") { onClose(); }
    // prevent ReactFlow from treating keystrokes as shortcuts
    e.stopPropagation();
  };

  return (
    <div
      className="nodrag nopan nowheel"
      onMouseDown={(e) => e.stopPropagation()}
      style={{
        position: "absolute",
        top: "calc(100% + 6px)",
        left: 0,
        zIndex: 1000,
        background: "var(--card)",
        border: "1px solid var(--border)",
        borderRadius: 8,
        padding: "10px 12px",
        display: "flex",
        flexDirection: "column",
        gap: 6,
        minWidth: 220,
        boxShadow: "var(--shadow-3)",
      }}
    >
      <input
        ref={labelRef}
        value={l}
        onChange={(e) => setL(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="Label"
        style={{
          background: "var(--muted)",
          border: "1px solid var(--border)",
          borderRadius: 5,
          padding: "4px 8px",
          fontSize: 12,
          color: "var(--foreground)",
          outline: "none",
          width: "100%",
          boxSizing: "border-box",
        }}
      />
      <input
        value={k}
        onChange={(e) => setK(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="Kind (e.g. component, service)"
        style={{
          background: "var(--muted)",
          border: "1px solid var(--border)",
          borderRadius: 5,
          padding: "4px 8px",
          fontSize: 12,
          color: "var(--foreground)",
          outline: "none",
          width: "100%",
          boxSizing: "border-box",
        }}
      />
      <textarea
        value={d}
        onChange={(e) => setD(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="Description (optional)"
        rows={2}
        style={{
          background: "var(--muted)",
          border: "1px solid var(--border)",
          borderRadius: 5,
          padding: "4px 8px",
          fontSize: 12,
          color: "var(--foreground)",
          outline: "none",
          resize: "none",
          width: "100%",
          boxSizing: "border-box",
          fontFamily: "inherit",
        }}
      />
      <div style={{ display: "flex", gap: 6, justifyContent: "flex-end" }}>
        <button
          type="button"
          onMouseDown={(e) => e.stopPropagation()}
          onClick={onClose}
          style={{
            fontSize: 11,
            padding: "3px 10px",
            borderRadius: 5,
            border: "1px solid var(--border)",
            background: "transparent",
            color: "var(--muted-foreground)",
            cursor: "pointer",
          }}
        >
          Cancel
        </button>
        <button
          type="button"
          onMouseDown={(e) => e.stopPropagation()}
          onClick={commit}
          style={{
            fontSize: 11,
            padding: "3px 10px",
            borderRadius: 5,
            border: "none",
            background: "var(--primary)",
            color: "var(--primary-foreground)",
            cursor: "pointer",
            fontWeight: 500,
          }}
        >
          Save
        </button>
      </div>
    </div>
  );
}

// ── Node component ────────────────────────────────────────────────────────────

function CanvasNodeComponent({ id, data, selected }: NodeProps<CanvasNodeData>) {
  const { label, kind, description, onEdit, onDelete } = data;
  const [editing, setEditing] = useState(false);
  const [hovered, setHovered] = useState(false);

  const handleSave = useCallback(
    (newLabel: string, newKind: string, newDesc: string) => {
      onEdit?.(id, newLabel, newKind, newDesc);
    },
    [id, onEdit]
  );

  const handleDelete = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onDelete?.(id);
    },
    [id, onDelete]
  );

  const showActions = hovered || selected || editing;

  return (
    <div
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onDoubleClick={() => setEditing(true)}
      style={{
        minWidth: 140,
        maxWidth: 220,
        display: "flex",
        flexDirection: "column",
        gap: 4,
        borderRadius: 8,
        border: `1px solid ${selected ? "var(--primary)" : "var(--border)"}`,
        background: "var(--card)",
        padding: "8px 12px",
        boxShadow: selected ? "0 0 0 1px var(--primary), var(--shadow-2)" : "var(--shadow-1)",
        fontSize: 13,
        cursor: "default",
        position: "relative",
        boxSizing: "border-box",
        transition: "box-shadow 0.12s, border-color 0.12s",
      }}
    >
      {/* Top (target) handle */}
      <Handle
        type="target"
        position={Position.Top}
        id="top"
        style={{
          width: 10,
          height: 10,
          background: "var(--card)",
          border: "2px solid var(--primary)",
          borderRadius: "50%",
          cursor: "crosshair",
        }}
      />

      {/* Delete button — visible on hover/select */}
      {showActions && onDelete && (
        <button
          type="button"
          className="nodrag"
          onMouseDown={(e) => e.stopPropagation()}
          onClick={handleDelete}
          style={{
            position: "absolute",
            top: -8,
            right: -8,
            width: 18,
            height: 18,
            borderRadius: "50%",
            border: "1px solid var(--border)",
            background: "var(--card)",
            color: "var(--destructive)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 11,
            cursor: "pointer",
            lineHeight: 1,
            padding: 0,
            zIndex: 10,
          }}
          title="Delete node"
        >
          ×
        </button>
      )}

      {/* Edit hint */}
      {showActions && !editing && (
        <div
          style={{
            position: "absolute",
            bottom: -18,
            left: "50%",
            transform: "translateX(-50%)",
            fontSize: 9,
            color: "var(--muted-foreground)",
            whiteSpace: "nowrap",
            pointerEvents: "none",
            opacity: 0.7,
          }}
        >
          double-click to edit
        </div>
      )}

      {/* Kind badge */}
      {kind && (
        <span
          style={{
            display: "inline-block",
            width: "fit-content",
            borderRadius: 4,
            padding: "1px 5px",
            fontSize: 10,
            fontWeight: 500,
            lineHeight: 1.4,
            background: "var(--muted)",
            color: "var(--muted-foreground)",
          }}
        >
          {kind}
        </span>
      )}

      {/* Label */}
      <span
        style={{
          fontWeight: 500,
          lineHeight: 1.35,
          color: "var(--foreground)",
          wordBreak: "break-word",
          userSelect: "none",
        }}
      >
        {label || <span style={{ color: "var(--muted-foreground)", fontStyle: "italic" }}>Untitled</span>}
      </span>

      {/* Description */}
      {description && (
        <span
          style={{
            fontSize: 11,
            lineHeight: 1.4,
            color: "var(--muted-foreground)",
            overflow: "hidden",
            display: "-webkit-box",
            WebkitLineClamp: 2,
            WebkitBoxOrient: "vertical",
            userSelect: "none",
          }}
        >
          {description}
        </span>
      )}

      {/* Inline edit popover */}
      {editing && (
        <EditPopover
          id={id}
          label={label}
          kind={kind ?? ""}
          description={description ?? ""}
          onSave={handleSave}
          onClose={() => setEditing(false)}
        />
      )}

      {/* Bottom (source) handle */}
      <Handle
        type="source"
        position={Position.Bottom}
        id="bottom"
        style={{
          width: 10,
          height: 10,
          background: "var(--card)",
          border: "2px solid var(--primary)",
          borderRadius: "50%",
          cursor: "crosshair",
        }}
      />

      {/* Side handles for richer connections */}
      <Handle
        type="source"
        position={Position.Right}
        id="right"
        style={{
          width: 10,
          height: 10,
          background: "var(--card)",
          border: "2px solid var(--primary)",
          borderRadius: "50%",
          cursor: "crosshair",
        }}
      />
      <Handle
        type="target"
        position={Position.Left}
        id="left"
        style={{
          width: 10,
          height: 10,
          background: "var(--card)",
          border: "2px solid var(--primary)",
          borderRadius: "50%",
          cursor: "crosshair",
        }}
      />
    </div>
  );
}

export const CanvasNode = memo(CanvasNodeComponent);
