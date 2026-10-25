// @ts-check
// Plan graph layout (.design/components.md, "Plan graph"), pure: phases → columns, records → nodes on one row
// grid shared by every column, dependencies → edge paths that never cross a node. No DOM, so node tests run it
// (tests/web/plan-layout.test.mjs).
(function () {
  "use strict";
  const COL = 296, ROW = 80, NODE_W = 184, NODE_H = 56, PAD_X = 24, TOP = 72, BOTTOM = 16;
  const GAP = COL - NODE_W;
  const ARROW = 6; // edges stop short of the node so the arrowhead shows

  /**
   * @typedef {{key: string, phase: string, deps: string[], blocked: boolean, done: boolean}} PlanIn
   * @typedef {{key: string, col: number, row: number, x: number, y: number}} PlanNode
   * @typedef {{from: string, to: string, d: string, style: "met" | "wait" | "block", label?: {x: number, y: number, text: string}}} PlanEdge
   */

  /** Dependency cycles (each as its keys, first repeated last); the edges that close them are dropped.
   *  @param {PlanIn[]} nodes @returns {{cycles: string[][], cut: Set<string>}} */
  function cycles(nodes) {
    const by = new Map(nodes.map(n => [n.key, n]));
    /** @type {Map<string, number>} 0 new, 1 on stack, 2 done */
    const mark = new Map();
    /** @type {string[][]} */
    const found = [];
    const cut = new Set();
    /** @type {string[]} */
    const stack = [];
    /** @param {string} k */
    const visit = k => {
      mark.set(k, 1);
      stack.push(k);
      for (const d of (by.get(k) || { deps: [] }).deps) {
        if (!by.has(d)) continue;
        const m = mark.get(d) || 0;
        if (m === 1) {
          found.push([...stack.slice(stack.indexOf(d)), d]);
          cut.add(`${d}>${k}`);
        } else if (m === 0) visit(d);
      }
      stack.pop();
      mark.set(k, 2);
    };
    for (const n of [...nodes].sort((a, b) => cmp(a.key, b.key))) if (!mark.get(n.key)) visit(n.key);
    return { cycles: found, cut };
  }

  /** @param {string} a @param {string} b */
  const cmp = (a, b) => String(a).localeCompare(String(b), undefined, { numeric: true });

  /**
   * Lays out `nodes` in `phases` order. Nodes whose phase isn't listed go in a last column.
   * @param {string[]} phases @param {PlanIn[]} input @param {{edges?: boolean}} [opts]
   */
  function layout(phases, input, opts = {}) {
    const cols = [...phases];
    if (input.some(n => !cols.includes(n.phase))) cols.push("");
    const colOf = (/** @type {PlanIn} */ n) => (cols.includes(n.phase) ? cols.indexOf(n.phase) : cols.length - 1);
    const { cycles: loops, cut } = cycles(input);
    const by = new Map(input.map(n => [n.key, n]));
    /** Deps that draw: known, not closing a cycle. @param {PlanIn} n */
    const depsOf = n => n.deps.filter(d => by.has(d) && d !== n.key && !cut.has(`${n.key}>${d}`) && !cut.has(`${d}>${n.key}`));
    /** @type {Map<string, PlanNode>} */
    const placed = new Map();
    /** @type {Set<string>[]} rows taken per column */
    const taken = cols.map(() => new Set());
    cols.forEach((_, c) => {
      const here = input.filter(n => colOf(n) === c);
      // Rows by barycenter of the deps already placed (earlier columns); ties and dep-less nodes by key.
      const want = new Map(here.map(n => {
        const rows = depsOf(n).map(d => placed.get(d)).filter(Boolean).map(p => /** @type {PlanNode} */ (p).row);
        return [n.key, rows.length ? rows.reduce((a, b) => a + b, 0) / rows.length : Infinity];
      }));
      here.sort((a, b) => (/** @type {number} */ (want.get(a.key)) - /** @type {number} */ (want.get(b.key))) || cmp(a.key, b.key));
      let next = 0;
      for (const n of here) {
        const w = /** @type {number} */ (want.get(n.key));
        const row = Number.isFinite(w) ? Math.max(Math.round(w), next) : next;
        next = row + 1;
        taken[c].add(String(row));
        placed.set(n.key, { key: n.key, col: c, row, x: PAD_X + c * COL, y: TOP + row * ROW });
      }
    });
    const rows = Math.max(1, ...[...placed.values()].map(p => p.row + 1));
    const width = PAD_X * 2 + (cols.length - 1) * COL + NODE_W;
    const height = TOP + rows * ROW - (ROW - NODE_H) + BOTTOM;
    // Blocked upstream: a blocked node, and everything downstream of one.
    const down = new Set();
    /** @param {string} k */
    const spread = k => { for (const n of input) if (depsOf(n).includes(k) && !down.has(n.key)) { down.add(n.key); spread(n.key); } };
    for (const n of input) if (n.blocked) spread(n.key);
    /** @type {PlanEdge[]} */
    const edges = [];
    if (opts.edges !== false) {
      for (const n of input) for (const d of depsOf(n)) {
        const s = /** @type {PlanNode} */ (placed.get(d)), t = /** @type {PlanNode} */ (placed.get(n.key)), src = /** @type {PlanIn} */ (by.get(d));
        const style = src.blocked || down.has(d) ? "block" : src.done ? "met" : "wait";
        edges.push({ from: d, to: n.key, style, ...route(s, t, taken, src.blocked ? `${d} holds ${n.key}` : "") });
      }
    }
    return { cols, nodes: [...placed.values()], edges, cycles: loops, width, height, downstream: down, taken };
  }

  /** The path from `s` to `t`, and where a label fits (an empty cell on its way, else the first gap).
   *  @param {PlanNode} s @param {PlanNode} t @param {Set<string>[]} taken @param {string} text */
  function route(s, t, taken, text) {
    const y1 = s.y + NODE_H / 2, y2 = t.y + NODE_H / 2;
    if (s.col === t.col) {
      // Same column: a short vertical edge between the two boxes.
      const x = s.x + NODE_W / 2, down = t.row > s.row;
      const d = `M${x} ${down ? s.y + NODE_H : s.y} L${x} ${down ? t.y - ARROW : t.y + NODE_H + ARROW}`;
      // A label sits just right of the line, centred between the two boxes.
      return { d, ...(text && { label: { x: x + 84, y: (s.y + t.y + NODE_H) / 2, text } }) };
    }
    const x1 = s.x + NODE_W, x2 = t.x - ARROW;
    if (t.col === s.col + 1 || t.col < s.col) {
      const mx = (x1 + x2) / 2;
      const d = y1 === y2 ? `M${x1} ${y1} L${x2} ${y2}` : `M${x1} ${y1} C${mx} ${y1} ${mx} ${y2} ${x2} ${y2}`;
      // Near the target's row, so two edges leaving one node keep their labels apart.
      return { d, ...(text && { label: { x: mx, y: y2 - 8, text } }) };
    }
    // Spanning columns: run along a row that's free in every column between, else between two rows.
    /** @type {number[]} */
    const between = [];
    for (let c = s.col + 1; c < t.col; c++) between.push(c);
    const free = (/** @type {number} */ r) => between.every(c => !taken[c].has(String(r)));
    const lane = free(s.row) ? s.y + NODE_H / 2 : free(t.row) ? t.y + NODE_H / 2
      : TOP + Math.min(s.row, t.row) * ROW + NODE_H + (ROW - NODE_H) / 2;
    const a = x1 + GAP / 2, b = x2 - GAP / 2 + ARROW / 2;
    let d = `M${x1} ${y1}`;
    d += lane === y1 ? ` L${x1 + GAP} ${lane}` : ` C${a} ${y1} ${a} ${lane} ${x1 + GAP} ${lane}`;
    d += ` L${x2 - GAP + ARROW} ${lane}`;
    d += lane === y2 ? ` L${x2} ${y2}` : ` C${b} ${lane} ${b} ${y2} ${x2} ${y2}`;
    // The label sits in the first empty cell the edge crosses, centred over the line.
    const c0 = between[0];
    return { d, ...(text && { label: { x: PAD_X + c0 * COL + NODE_W / 2, y: lane - 10, text } }) };
  }

  /** True when the segment-wise path `d` (M/L/C points) passes through a node box, for tests.
   *  @param {string} d @param {PlanNode[]} nodes @param {string[]} skip */
  function crosses(d, nodes, skip) {
    const pts = (d.match(/-?\d+(\.\d+)?/g) || []).map(Number);
    /** @type {[number, number][]} */
    const xy = [];
    for (let i = 0; i + 1 < pts.length; i += 2) xy.push([pts[i], pts[i + 1]]);
    // Sample the polyline through every point (control points included: a conservative test).
    for (let i = 0; i + 1 < xy.length; i++) {
      for (let s = 0; s <= 20; s++) {
        const x = xy[i][0] + (xy[i + 1][0] - xy[i][0]) * s / 20, y = xy[i][1] + (xy[i + 1][1] - xy[i][1]) * s / 20;
        for (const n of nodes) {
          if (skip.includes(n.key)) continue;
          if (x > n.x + 1 && x < n.x + NODE_W - 1 && y > n.y + 1 && y < n.y + NODE_H - 1) return n.key;
        }
      }
    }
    return null;
  }

  const api = { layout, route, cycles, crosses, COL, ROW, NODE_W, NODE_H, PAD_X, TOP, GAP };
  /** @type {any} */ (globalThis).MissionPlanLayout = api;
})();
