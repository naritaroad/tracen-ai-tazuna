// The motion graph of one motion set: loops are nodes, transition clips are edges.
// Pure functions (no three.js), so the planner and the tests can use them directly.

import type { MotionClip, MotionSet } from "./vocab";

export interface MotionGraph {
  rest: string;
  clips: Map<string, MotionClip>;
  /** Transition clips leaving each node. */
  edges: Map<string, MotionClip[]>;
}

export function buildGraph(set: MotionSet): MotionGraph {
  const clips = new Map(set.clips.map((c) => [c.id, c]));
  const edges = new Map<string, MotionClip[]>();
  for (const c of set.clips) {
    if (c.loop) continue;
    edges.set(c.from, [...(edges.get(c.from) ?? []), c]);
  }
  return { rest: set.rest, clips, edges };
}

export function isLoop(graph: MotionGraph, id: string): boolean {
  return graph.clips.get(id)?.loop === true;
}

/**
 * The transition clips that lead from node `from` to node `to`, fewest first (breadth-first).
 * `[]` when already there; `null` when no chain of clips connects them.
 */
export function findPath(graph: MotionGraph, from: string, to: string): string[] | null {
  if (from === to) return [];
  const prev = new Map<string, { node: string; clip: string }>();
  const seen = new Set([from]);
  const queue = [from];
  while (queue.length) {
    const node = queue.shift()!;
    for (const edge of graph.edges.get(node) ?? []) {
      if (seen.has(edge.to)) continue;
      seen.add(edge.to);
      prev.set(edge.to, { node, clip: edge.id });
      if (edge.to === to) {
        const path: string[] = [];
        for (let at = to; at !== from; ) {
          const step = prev.get(at)!;
          path.unshift(step.clip);
          at = step.node;
        }
        return path;
      }
      queue.push(edge.to);
    }
  }
  return null;
}

/** Loops tagged with `tag` (a gesture or `emotion:<name>`) that can be reached from the rest loop. */
export function loopsTagged(graph: MotionGraph, tag: string): MotionClip[] {
  return [...graph.clips.values()].filter((c) => c.loop && c.gestures.includes(tag) && (c.id === graph.rest || findPath(graph, graph.rest, c.id) !== null));
}
