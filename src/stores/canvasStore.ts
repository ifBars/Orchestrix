/**
 * canvasStore.ts
 *
 * Lightweight Zustand store for canvas UI state that needs to be shared
 * across components (primarily selected node context for the Composer).
 *
 * Kept separate from appStore to avoid re-renders in unrelated components.
 */

import { create } from "zustand";
import type { CanvasNode } from "@/types";

type CanvasStoreState = {
  /** The task whose canvas nodes are currently selected. */
  activeTaskId: string | null;
  /** The full CanvasNode objects for the currently selected nodes. */
  selectedNodes: CanvasNode[];

  setSelectedNodes: (taskId: string, nodes: CanvasNode[]) => void;
  clearSelection: () => void;
};

export const useCanvasStore = create<CanvasStoreState>((set) => ({
  activeTaskId: null,
  selectedNodes: [],

  setSelectedNodes: (taskId, nodes) =>
    set({ activeTaskId: taskId, selectedNodes: nodes }),

  clearSelection: () =>
    set({ activeTaskId: null, selectedNodes: [] }),
}));
