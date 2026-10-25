// Unit tests for the plan graph layout (web/plan-layout.js), in plain Node: rows by barycenter, edge styles,
// no edge crossing a node, cycles reported instead of hanging. Run: `node tests/web/plan-layout.test.mjs`.

import assert from "node:assert/strict";
import { test } from "node:test";
await import("../../web/plan-layout.js");
const L = globalThis.MissionPlanLayout;

const node = (key, phase, deps = [], state = "todo") =>
  ({ key, phase, deps, blocked: state === "blocked", done: ["merged", "done"].includes(state) });

/** The demo mission's plan (tests/browser/demo.mjs, the mockups' scenario). */
const DEMO = [
  node("C0", "P1", [], "merged"), node("C1", "P1", [], "done"),
  node("A1", "P2", ["C0"], "merged"), node("S1", "P2", ["C0"], "blocked"), node("U1", "P2", ["C1"], "in_review"),
  node("J1", "P3", ["A1"], "in_progress"), node("D1", "P3", ["U1"]),
  node("R1", "P4", ["J1", "D1", "S1"]),
];
const PHASES = ["P1", "P2", "P3", "P4"];
const rowOf = (g, k) => g.nodes.find((n) => n.key === k).row;

test("rows follow the barycenter of each node's dependencies", () => {
  const g = L.layout(PHASES, DEMO);
  const rows = Object.fromEntries(DEMO.map((n) => [n.key, rowOf(g, n.key)]));
  assert.deepEqual(rows, { C0: 0, C1: 1, A1: 0, S1: 1, U1: 2, J1: 0, D1: 2, R1: 1 });
  assert.equal(g.nodes.find((n) => n.key === "R1").x, L.PAD_X + 3 * L.COL);
});

test("edges are styled by their source: met, waiting, blocked upstream", () => {
  const g = L.layout(PHASES, DEMO);
  const style = (f, t) => g.edges.find((e) => e.from === f && e.to === t).style;
  assert.equal(style("C0", "A1"), "met");
  assert.equal(style("J1", "R1"), "wait");
  assert.equal(style("S1", "R1"), "block");
  assert.equal(g.edges.find((e) => e.from === "S1").label.text, "S1 holds R1");
  assert.deepEqual([...g.downstream], ["R1"]);
});

test("no edge crosses a node, in the demo and in denser fixtures", () => {
  const fixtures = [DEMO];
  // Every node in a later phase depends on every node two phases back: long edges over full columns.
  const dense = [];
  for (let p = 0; p < 5; p++) for (let i = 0; i < 4; i++) {
    dense.push(node(`N${p}${i}`, `P${p}`, p >= 2 ? [0, 1, 2, 3].map((j) => `N${p - 2}${j}`) : p === 1 ? [`N0${i}`] : []));
  }
  fixtures.push(dense);
  fixtures.push([node("A", "P1"), node("B", "P1", ["A"]), node("C", "P3", ["A"]), node("D", "P2", ["B"]), node("E", "P2")]);
  const g0 = L.layout(PHASES, DEMO), c0 = g0.nodes.find((n) => n.key === "J1");
  assert.equal(L.crosses(`M0 ${c0.y + 20} L2000 ${c0.y + 20}`, g0.nodes, []), "C0", "the check itself sees a crossing");
  for (const f of fixtures) {
    const g = L.layout([...new Set(f.map((n) => n.phase))].sort(), f);
    for (const e of g.edges) assert.equal(L.crosses(e.d, g.nodes, [e.from, e.to]), null, `${e.from}→${e.to} ${e.d}`);
  }
});

test("a dependency cycle is reported and its closing edge dropped, without hanging", () => {
  const g = L.layout(["P1"], [node("A", "P1", ["B"]), node("B", "P1", ["C"]), node("C", "P1", ["A"])]);
  assert.equal(g.cycles.length, 1);
  assert.deepEqual(g.cycles[0].slice(0, 1), g.cycles[0].slice(-1), "the cycle names its first key last too");
  assert.equal(g.edges.length, 2);
});

test("nodes in an unknown phase get a last column; flow-style layouts draw no edges", () => {
  const g = L.layout(["P1"], [node("A", "P1"), node("B", "P9", ["A"])], { edges: false });
  assert.deepEqual(g.cols, ["P1", ""]);
  assert.equal(g.edges.length, 0);
});
