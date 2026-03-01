/**
 * canvasLayout.ts
 *
 * Dagre-based auto-layout for the architecture canvas.
 *
 * Used when the AI adds nodes without explicit x/y coordinates — the
 * frontend detects missing positions and runs this layout on those nodes
 * before handing them to React Flow.
 */

import dagre from "dagre";
import type { Node, Edge } from "reactflow";

export const NODE_WIDTH = 160;
export const NODE_HEIGHT = 52;

export type LayoutDirection = "TB" | "LR";

/**
 * Runs a dagre layout over the provided React Flow nodes + edges.
 *
 * Nodes that already have an explicit position (x !== undefined) are
 * pinned — dagre still needs them in the graph to route edges correctly,
 * but their output position is discarded and the original is kept.
 *
 * Returns a new array of nodes with updated positions for previously
 * un-positioned nodes.
 */
export function applyDagreLayout(
  nodes: Node[],
  edges: Edge[],
  direction: LayoutDirection = "TB",
): Node[] {
  const dagreGraph = new dagre.graphlib.Graph();
  dagreGraph.setDefaultEdgeLabel(() => ({}));
  dagreGraph.setGraph({ rankdir: direction, nodesep: 48, ranksep: 64 });

  for (const node of nodes) {
    dagreGraph.setNode(node.id, {
      width: node.width ?? NODE_WIDTH,
      height: node.height ?? NODE_HEIGHT,
    });
  }

  for (const edge of edges) {
    dagreGraph.setEdge(edge.source, edge.target);
  }

  dagre.layout(dagreGraph);

  return nodes.map((node) => {
    // Keep manually-placed nodes at their existing position
    const hasPosition =
      node.position.x !== 0 || node.position.y !== 0 || node.data?.positionSet === true;

    if (hasPosition && node.data?.positionSet === true) {
      return node;
    }

    const nodeWithPosition = dagreGraph.node(node.id);
    return {
      ...node,
      position: {
        x: nodeWithPosition.x - (node.width ?? NODE_WIDTH) / 2,
        y: nodeWithPosition.y - (node.height ?? NODE_HEIGHT) / 2,
      },
      data: { ...node.data, positionSet: true },
    };
  });
}

/**
 * Given a set of React Flow nodes, returns a new list where all nodes
 * that lack a `positionSet` flag are repositioned by dagre, and nodes
 * already positioned by the user are left in place.
 */
export function layoutUnpositioned(nodes: Node[], edges: Edge[]): Node[] {
  const hasUnpositioned = nodes.some((n) => !n.data?.positionSet);
  if (!hasUnpositioned) return nodes;
  return applyDagreLayout(nodes, edges);
}
