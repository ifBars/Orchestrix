/**
 * ArchitectureCanvas.tsx
 *
 * The main Architecture Canvas surface.
 *
 * Responsibilities:
 * - Loads the persisted canvas state for the current task via Tauri IPC.
 * - Subscribes to `canvas.updated` events and reflects AI mutations in real time.
 * - Lets the user add/drag nodes and save the state back via `update_task_canvas`.
 * - Auto-lays out AI-added nodes (no x/y) using dagre.
 * - Pushes selected node data into `canvasStore` so the Composer can inject
 *   node context into outgoing messages (Phase 4).
 *
 * Design:
 * - Uses React Flow v11 with a custom `CanvasNode` type.
 * - Minimal toolbar: auto-layout, add node, selection indicator.
 * - Background, MiniMap, and Controls are standard React Flow utilities.
 * - Follows the Orchestrix design system tokens (see DESIGN_SYSTEM.md).
 */

import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import ReactFlow, {
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  addEdge,
  useEdgesState,
  useNodesState,
  type Connection,
  type Node,
  type Edge,
  type OnSelectionChangeParams,
} from "reactflow";
import "reactflow/dist/style.css";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { CanvasNode, type CanvasNodeData } from "./CanvasNode";
import { layoutUnpositioned, NODE_WIDTH, NODE_HEIGHT } from "./canvasLayout";
import { useCanvasStore } from "@/stores/canvasStore";
import type { CanvasNode as CanvasNodeType, CanvasState, CanvasUpdatedPayload, TaskCanvasRow } from "@/types";

// ── React Flow node type registry ────────────────────────────────────────────

const NODE_TYPES = { canvasNode: CanvasNode };

// ── Helpers ───────────────────────────────────────────────────────────────────

function parseCanvasState(stateJson: string): CanvasState {
  try {
    const raw = JSON.parse(stateJson) as {
      nodes?: unknown[];
      edges?: unknown[];
    };

    // Normalize nodes: the Rust handler used to store nodes as
    // { id, type, data: { label, description } } — flatten to the canonical
    // CanvasNode shape { id, label, kind, description, x, y }.
    const nodes: CanvasState["nodes"] = (raw.nodes ?? []).map((n: unknown) => {
      const node = n as Record<string, unknown>;
      const nestedData = node.data as Record<string, unknown> | undefined;

      const label =
        (typeof node.label === "string" && node.label) ||
        (typeof nestedData?.label === "string" && nestedData.label) ||
        "";

      const kind =
        (typeof node.kind === "string" && node.kind) ||
        (typeof node.type === "string" && node.type !== "canvasNode" && node.type) ||
        undefined;

      const description =
        (typeof node.description === "string" && node.description) ||
        (typeof nestedData?.description === "string" && nestedData.description) ||
        undefined;

      return {
        id: String(node.id ?? ""),
        label,
        kind: kind || undefined,
        description: description || undefined,
        x: typeof node.x === "number" ? node.x : undefined,
        y: typeof node.y === "number" ? node.y : undefined,
        width: typeof node.width === "number" ? node.width : undefined,
        height: typeof node.height === "number" ? node.height : undefined,
      };
    });

    const edges: CanvasState["edges"] = (raw.edges ?? []).map((e: unknown) => {
      const edge = e as Record<string, unknown>;
      return {
        id: String(edge.id ?? ""),
        source: String(edge.source ?? ""),
        target: String(edge.target ?? ""),
        label: typeof edge.label === "string" ? edge.label : undefined,
      };
    });

    return { nodes, edges };
  } catch {
    return { nodes: [], edges: [] };
  }
}

function canvasStatesToFlowNodes(state: CanvasState): Node<CanvasNodeData>[] {
  return state.nodes.map((n) => ({
    id: n.id,
    type: "canvasNode",
    position: { x: n.x ?? 0, y: n.y ?? 0 },
    width: n.width ?? NODE_WIDTH,
    height: n.height ?? NODE_HEIGHT,
    data: {
      label: n.label,
      kind: n.kind,
      description: n.description,
      // Nodes without x/y from the source need layout
      positionSet: n.x !== undefined && n.y !== undefined,
    },
  }));
}

function canvasStatesToFlowEdges(state: CanvasState): Edge[] {
  return state.edges.map((e) => ({
    id: e.id,
    source: e.source,
    target: e.target,
    label: e.label,
    type: "smoothstep",
    style: { strokeWidth: 1.5 },
  }));
}

function flowNodesToCanvasState(
  nodes: Node<CanvasNodeData>[],
  edges: Edge[],
): CanvasState {
  return {
    nodes: nodes.map((n) => ({
      id: n.id,
      label: n.data.label,
      kind: n.data.kind,
      description: n.data.description,
      x: n.position.x,
      y: n.position.y,
      width: n.width ?? NODE_WIDTH,
      height: n.height ?? NODE_HEIGHT,
    })),
    edges: edges.map((e) => ({
      id: e.id,
      source: e.source,
      target: e.target,
      label: typeof e.label === "string" ? e.label : undefined,
    })),
  };
}

/** Convert a React Flow node to the canonical CanvasNode shape for the store. */
function rfNodeToCanvasNode(n: Node<CanvasNodeData>): CanvasNodeType {
  return {
    id: n.id,
    label: n.data.label,
    kind: n.data.kind,
    description: n.data.description,
    x: n.position.x,
    y: n.position.y,
  };
}

// ── Unique ID helper ──────────────────────────────────────────────────────────

let _nodeCounter = 0;
function newNodeId(): string {
  return `node-${Date.now()}-${++_nodeCounter}`;
}

// ── Component ─────────────────────────────────────────────────────────────────

type ArchitectureCanvasProps = {
  taskId: string;
};

export function ArchitectureCanvas({ taskId }: ArchitectureCanvasProps) {
  const [nodes, setNodes, onNodesChange] = useNodesState<CanvasNodeData>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState([]);
  const [loading, setLoading] = useState(true);
  const [selectedNodeIds, setSelectedNodeIds] = useState<string[]>([]);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Track whether the current change came from a remote event (so we don't echo it back)
  const suppressSaveRef = useRef(false);

  const setStoreSelectedNodes = useCanvasStore((s) => s.setSelectedNodes);
  const clearSelection = useCanvasStore((s) => s.clearSelection);

  // ── Clear selection when unmounted or task changes ─────────────────────────

  useEffect(() => {
    return () => { clearSelection(); };
  }, [taskId, clearSelection]);

  // ── Load initial state ─────────────────────────────────────────────────────

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    clearSelection();

    invoke<TaskCanvasRow | null>("get_task_canvas", { taskId })
      .then((row) => {
        if (cancelled) return;
        if (row) {
          const state = parseCanvasState(row.state_json);
          const rfNodes = canvasStatesToFlowNodes(state);
          const rfEdges = canvasStatesToFlowEdges(state);
          const laid = layoutUnpositioned(rfNodes, rfEdges);
          suppressSaveRef.current = true;
          setNodes(laid);
          setEdges(rfEdges);
          setTimeout(() => { suppressSaveRef.current = false; }, 0);
        }
      })
      .catch(console.error)
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [taskId, setNodes, setEdges, clearSelection]);

  // ── Listen for canvas.updated events from the backend ─────────────────────

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    listen<CanvasUpdatedPayload>("canvas.updated", (event) => {
      if (event.payload.task_id !== taskId) return;
      const state = parseCanvasState(event.payload.state_json);
      const rfNodes = canvasStatesToFlowNodes(state);
      const rfEdges = canvasStatesToFlowEdges(state);
      const laid = layoutUnpositioned(rfNodes, rfEdges);
      suppressSaveRef.current = true;
      setNodes(laid);
      setEdges(rfEdges);
      setTimeout(() => { suppressSaveRef.current = false; }, 0);
    })
      .then((fn) => { unlisten = fn; })
      .catch(console.error);

    return () => { unlisten?.(); };
  }, [taskId, setNodes, setEdges]);

  // ── Debounced save ────────────────────────────────────────────────────────

  const scheduleSave = useCallback(
    (currentNodes: Node<CanvasNodeData>[], currentEdges: Edge[]) => {
      if (suppressSaveRef.current) return;
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      saveTimerRef.current = setTimeout(() => {
        const stateJson = JSON.stringify(
          flowNodesToCanvasState(currentNodes, currentEdges)
        );
        invoke("update_task_canvas", { taskId, stateJson }).catch(console.error);
      }, 600);
    },
    [taskId]
  );

  // Wrap change handlers to trigger save
  const handleNodesChange: typeof onNodesChange = useCallback(
    (changes) => {
      onNodesChange(changes);
      // We need the post-change state; React Flow updates are synchronous on next tick
      setNodes((nds) => {
        scheduleSave(nds, edges);
        return nds;
      });
    },
    [onNodesChange, scheduleSave, edges, setNodes]
  );

  const handleEdgesChange: typeof onEdgesChange = useCallback(
    (changes) => {
      onEdgesChange(changes);
      setEdges((eds) => {
        scheduleSave(nodes, eds);
        return eds;
      });
    },
    [onEdgesChange, scheduleSave, nodes, setEdges]
  );

  const handleConnect = useCallback(
    (params: Connection) => {
      const edge: Edge = {
        ...params,
        id: `edge-${params.source}-${params.target}-${Date.now()}`,
        type: "smoothstep",
        style: { strokeWidth: 1.5 },
      } as Edge;
      setEdges((eds) => {
        const next = addEdge(edge, eds);
        scheduleSave(nodes, next);
        return next;
      });
    },
    [setEdges, scheduleSave, nodes]
  );

  // ── Selection → canvasStore (Phase 4) ─────────────────────────────────────

  const handleSelectionChange = useCallback(
    ({ nodes: selected }: OnSelectionChangeParams) => {
      setSelectedNodeIds(selected.map((n) => n.id));
      if (selected.length === 0) {
        clearSelection();
      } else {
        setStoreSelectedNodes(taskId, selected.map(rfNodeToCanvasNode));
      }
    },
    [taskId, setStoreSelectedNodes, clearSelection]
  );

  // ── Edit node (called from CanvasNode inline editor) ──────────────────────

  const handleEditNode = useCallback(
    (nodeId: string, label: string, kind: string, description: string) => {
      setNodes((nds) => {
        const next = nds.map((n) =>
          n.id === nodeId
            ? { ...n, data: { ...n.data, label, kind: kind || undefined, description: description || undefined } }
            : n
        );
        scheduleSave(next, edges);
        return next;
      });
    },
    [setNodes, scheduleSave, edges]
  );

  // ── Delete node ───────────────────────────────────────────────────────────

  const deleteNodes = useCallback(
    (ids: string[]) => {
      if (ids.length === 0) return;
      const idSet = new Set(ids);
      setNodes((nds) => {
        const next = nds.filter((n) => !idSet.has(n.id));
        setEdges((eds) => {
          const nextEdges = eds.filter(
            (e) => !idSet.has(e.source) && !idSet.has(e.target)
          );
          scheduleSave(next, nextEdges);
          return nextEdges;
        });
        return next;
      });
      clearSelection();
      setSelectedNodeIds([]);
    },
    [setNodes, setEdges, scheduleSave, clearSelection]
  );

  const handleDeleteNode = useCallback(
    (nodeId: string) => deleteNodes([nodeId]),
    [deleteNodes]
  );

  const deleteSelected = useCallback(
    () => deleteNodes(selectedNodeIds),
    [deleteNodes, selectedNodeIds]
  );

  // ── Inject callbacks into node data ───────────────────────────────────────
  // React Flow re-renders nodes when data changes. We inject stable callbacks
  // as part of node data so CanvasNode can call them without prop-drilling.

  const nodesWithCallbacks = useMemo(
    () =>
      nodes.map((n) => ({
        ...n,
        data: {
          ...n.data,
          onEdit: handleEditNode,
          onDelete: handleDeleteNode,
        },
      })),
    [nodes, handleEditNode, handleDeleteNode]
  );

  const addNode = useCallback(() => {
    const id = newNodeId();
    const newNode: Node<CanvasNodeData> = {
      id,
      type: "canvasNode",
      position: {
        x: 80 + Math.random() * 200,
        y: 80 + Math.random() * 200,
      },
      data: { label: "New node", positionSet: true },
    };
    setNodes((nds) => {
      const next = [...nds, newNode];
      scheduleSave(next, edges);
      return next;
    });
  }, [setNodes, scheduleSave, edges]);

  // ── Auto-layout ───────────────────────────────────────────────────────────

  const autoLayout = useCallback(() => {
    setNodes((nds) => {
      // Reset positionSet so dagre repositions everything
      const reset = nds.map((n) => ({ ...n, data: { ...n.data, positionSet: false } }));
      const laid = layoutUnpositioned(reset, edges);
      scheduleSave(laid, edges);
      return laid;
    });
  }, [setNodes, edges, scheduleSave]);

  // ── MiniMap node color helper ─────────────────────────────────────────────

  const minimapNodeColor = useCallback(
    (node: Node<CanvasNodeData>) => {
      if (node.selected) return "var(--primary)";
      return "var(--muted-foreground)";
    },
    []
  );

  // ── Render ────────────────────────────────────────────────────────────────

  return (
    <div className="relative flex h-full w-full flex-col overflow-hidden rounded-lg border border-border/70 bg-background">
      {/* Toolbar */}
      <div className="flex shrink-0 items-center gap-2 border-b border-border/70 bg-card/80 px-3 py-1.5 backdrop-blur-md">
        <span className="text-xs font-medium text-muted-foreground">Architecture Canvas</span>
        <div className="ml-auto flex items-center gap-1.5">
          {selectedNodeIds.length > 0 && (
            <button
              type="button"
              onClick={deleteSelected}
              className="rounded px-2 py-1 text-xs font-medium text-destructive transition-colors hover:bg-destructive/10 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
              title="Delete selected nodes"
            >
              Delete Selected ({selectedNodeIds.length})
            </button>
          )}
          <button
            type="button"
            onClick={autoLayout}
            className="rounded px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
            title="Auto-layout (dagre)"
          >
            Auto-layout
          </button>
          <button
            type="button"
            onClick={addNode}
            className="rounded bg-primary/10 px-2 py-1 text-xs font-medium text-primary transition-colors hover:bg-primary/20 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
            title="Add node"
          >
            + Node
          </button>
        </div>
      </div>

      {/* Canvas */}
      <div className="relative min-h-0 flex-1">
        {loading && (
          <div className="absolute inset-0 z-10 flex items-center justify-center bg-background/60 text-sm text-muted-foreground">
            Loading canvas…
          </div>
        )}
        <ReactFlow
          nodes={nodesWithCallbacks}
          edges={edges}
          nodeTypes={NODE_TYPES}
          onNodesChange={handleNodesChange}
          onEdgesChange={handleEdgesChange}
          onConnect={handleConnect}
          onSelectionChange={handleSelectionChange}
          fitView
          fitViewOptions={{ padding: 0.2 }}
          minZoom={0.25}
          maxZoom={2}
          deleteKeyCode="Delete"
          proOptions={{ hideAttribution: true }}
        >
          <Background
            variant={BackgroundVariant.Dots}
            gap={20}
            size={1}
            color="var(--border)"
            style={{ opacity: 0.5 }}
          />
          <Controls
            className="!border-border/70 !bg-card/90 !shadow-none"
            showInteractive={false}
          />
          <MiniMap
            nodeColor={minimapNodeColor}
            maskColor="var(--background)"
            className="!border-border/70 !bg-card/90"
            style={{ height: 80 }}
          />
        </ReactFlow>
      </div>
    </div>
  );
}
