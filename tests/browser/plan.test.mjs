// The plan graph on the demo mission (tests/browser/demo.mjs): edge styles, gates, the current phase, the
// legend, accessible names, Trace blocked path, the Table / Board views, j/k, plus the phase track and the
// strip's phase cell on Overview; `flow` as an edgeless alias; an old run in waves (import) still drawing;
// a dependency cycle reported instead of hanging. Run: `node tests/browser/plan.test.mjs` after a release build.

import assert from "node:assert/strict";
import { join } from "node:path";
import { Agent, Browser, steps } from "./harness.mjs";
import { seedDemo } from "./demo.mjs";

const agent = new Agent();
let browser, demo;
const EVIL = `<img src=x onerror="window.pwned=1">`;
const q = (sel) => JSON.stringify(sel);
const text = (sel) => browser.eval(`document.querySelector(${q(sel)})?.textContent.replace(/\\s+/g, " ").trim()`);

await steps([
  ["the demo mission", async () => {
    await agent.start();
    demo = seedDemo(agent);
    browser = await Browser.launch({ width: 1440, height: 900 });
    await browser.goto(`${demo.url}/plan`);
    await browser.waitFor(`document.querySelectorAll("#plan .pnode").length === 8`);
  }],

  ["edges are drawn by their source's state, with a label on the blocked one", async () => {
    const n = (c) => browser.eval(`document.querySelectorAll("#plan .pedges path.${c}").length`);
    assert.equal(await n("e-met"), 4, "C0→A1, C0→S1, C1→U1, A1→J1");
    assert.equal(await n("e-wait"), 3, "U1→D1, J1→R1, D1→R1");
    assert.equal(await n("e-block"), 1, "S1→R1");
    assert.equal(await text("#plan .plabel"), "S1 holds R1");
    assert.equal(await browser.eval(`!!document.querySelector("#plan marker path.ah-b")`), true);
    assert.equal(await browser.eval(`getComputedStyle(document.querySelector("#plan marker path.ah-m")).fill !== "none"`), true, "arrowheads are filled");
  }],

  ["gates sit between phases in their tone; the current phase is banded", async () => {
    const gates = await browser.eval(`[...document.querySelectorAll("#plan .pgate .mono")].map(g => [g.textContent, g.className])`);
    assert.deepEqual(gates, [["G0 2/2", "mono t-done"], ["G1 1/3", "mono t-warn"], ["G2 0/2", "mono"]]);
    assert.equal(await text("#plan .phead.now .lbl"), "Phase 3 · Run · now");
    assert.equal(await browser.eval(`document.querySelectorAll("#plan .pband").length`), 1);
    assert.equal(await text("#plan h2 .cnt"), "3/8 done · 1 blocked");
  }],

  ["nodes: glyph, key and title, a sub-line that says why, and an accessible name with deps", async () => {
    const node = (k) => `#plan .pnode[data-record="packages/${k}"]`;
    assert.equal(await browser.eval(`document.querySelector(${q(node("S1"))}).getAttribute("aria-label")`), "S1 Storage adapter, blocked, depends on C0, holds R1");
    assert.equal(await browser.eval(`document.querySelector(${q(node("S1"))}).classList.contains("blk")`), true);
    assert.match(await text(`${node("U1")} small`), /in review · u1-ui · #421 · needs you/);
    assert.equal(await text(`${node("D1")} small`), "todo · waits on U1");
    assert.equal(await text(`${node("R1")} small`), "todo · waits on 3");
    assert.equal(await browser.eval(`document.querySelector(${q(node("S1"))}).getAttribute("href")`), "?record=packages%2FS1");
    assert.equal(await browser.eval(`document.querySelector("#plan .pedges").getAttribute("aria-hidden")`), "true");
  }],

  ["the legend is always there: five glyphs, three edge styles, the gate and the current phase", async () => {
    const legend = await text("#plan .plegend");
    for (const w of ["done", "in progress", "in review", "blocked", "todo", "met", "waiting", "blocked upstream", "gate", "current phase"]) assert.ok(legend.includes(w), w);
    assert.equal(await browser.eval(`document.querySelectorAll("#plan .plegend svg.g").length`), 5);
  }],

  ["Trace blocked path dims everything off the blocked node's downstream path", async () => {
    await browser.eval(`document.querySelector("#plan [data-plan-trace]").click()`);
    await browser.waitFor(`document.querySelector("#plan [data-plan-trace]").getAttribute("aria-pressed") === "true"`);
    const lit = await browser.eval(`[...document.querySelectorAll("#plan .pnode:not(.dim)")].map(n => n.dataset.record.split("/")[1])`);
    assert.deepEqual(lit, ["S1", "R1"]);
    await browser.eval(`document.querySelector("#plan [data-plan-trace]").click()`);
    await browser.waitFor(`!document.querySelector("#plan .dim")`);
  }],

  ["j and k walk the nodes in phase, then row order", async () => {
    await browser.eval(`document.querySelector('#plan .pnode[data-record="packages/C0"]').focus()`);
    await browser.key("j");
    assert.equal(await browser.eval(`document.activeElement.dataset.record`), "packages/C1");
    await browser.key("j");
    assert.equal(await browser.eval(`document.activeElement.dataset.record`), "packages/A1");
    await browser.key("k");
    assert.equal(await browser.eval(`document.activeElement.dataset.record`), "packages/C1");
  }],

  ["Table and Board show the same records, and the choice survives a reload", async () => {
    await browser.eval(`document.querySelector('#plan [data-plan-view="table"]').click()`);
    await browser.waitFor(`document.querySelectorAll("#plan tbody tr").length === 8`);
    assert.equal(await browser.eval(`[...document.querySelectorAll('#plan tr[data-key="R1"] td')].slice(0, 5).map(td => td.textContent).join("|")`), "R1|Release|P4|todo|J1, D1, S1");
    await browser.goto(`${demo.url}/plan`);
    await browser.waitFor(`document.querySelectorAll("#plan tbody tr").length === 8`);
    await browser.eval(`document.querySelector('#plan [data-plan-view="board"]').click()`);
    await browser.waitFor(`document.querySelectorAll("#plan .kcard").length === 8`);
    await browser.eval(`document.querySelector('#plan [data-plan-view="graph"]').click()`);
    await browser.waitFor(`document.querySelectorAll("#plan .pnode").length === 8`);
  }],

  ["zoom, fit, pan and fullscreen; fullscreen survives a live update", async () => {
    const zoom = () => browser.eval(`Number(document.querySelector("#plan .vcanvas").style.zoom)`);
    await browser.eval(`document.querySelector('#plan [data-vz="in"]').click()`);
    assert.equal(await zoom(), 1.2);
    assert.equal(await text("#plan .vlevel"), "120%");
    await browser.eval(`document.querySelector("#plan [data-viewport]").focus()`);
    await browser.key("+");
    assert.equal(await zoom(), 1.44);
    // Drag the background to pan.
    const box = await browser.eval(`(() => { const r = document.querySelector("#plan [data-viewport]").getBoundingClientRect(); return [r.left + 20, r.top + 20]; })()`);
    const before = await browser.eval(`document.querySelector("#plan [data-viewport]").scrollLeft`);
    await browser.send("Input.dispatchMouseEvent", { type: "mousePressed", x: box[0] + 200, y: box[1], button: "left", clickCount: 1 });
    await browser.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: box[0] + 60, y: box[1], button: "left" });
    await browser.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: box[0] + 60, y: box[1], button: "left" });
    assert.ok((await browser.eval(`document.querySelector("#plan [data-viewport]").scrollLeft`)) > before, "dragging left scrolls right");
    await browser.key("0");
    assert.ok((await zoom()) <= 1, "fit");
    // f (a real key press, so the browser allows fullscreen) puts the section in fullscreen.
    await browser.key("f");
    await browser.waitFor(`document.fullscreenElement?.id === "plan" || document.getElementById("plan").classList.contains("vfull")`);
    agent.run("upsert", "-m", demo.id, "packages", "D1", "note=Draft started");
    await browser.waitFor(`document.querySelector('#plan .pnode[data-record="packages/D1"]') && (document.fullscreenElement?.id === "plan" || document.getElementById("plan").classList.contains("vfull"))`);
    await browser.eval(`document.fullscreenElement ? document.exitFullscreen() : document.getElementById("plan").classList.remove("vfull")`);
    await browser.waitFor(`!document.fullscreenElement && !document.querySelector(".vfull")`);
  }],

  ["at 390 px the graph scrolls inside its card, never the page", async () => {
    const phone = await Browser.launch({ width: 390, height: 844, mobile: true });
    try {
      await phone.goto(`${demo.url}/plan`);
      await phone.waitFor(`document.querySelectorAll("#plan .pnode").length === 8`);
      assert.equal(await phone.eval(`document.documentElement.scrollWidth <= innerWidth`), true);
      assert.equal(await phone.eval(`(() => { const s = document.querySelector("#plan .pscroll"); return s.scrollWidth > s.clientWidth; })()`), true);
      assert.equal(await phone.eval(`document.querySelector("#plan .pnode").getBoundingClientRect().width`), 184);
    } finally { await phone.close(); }
  }],

  ["Overview: the phase track and the strip's phase cell read the same plan", async () => {
    await browser.goto(demo.url);
    await browser.waitFor(`document.querySelectorAll("#track .tphase").length === 4`);
    assert.equal(await text("#track .tphase.now .lbl"), "Phase 3 · now");
    assert.match(await text("#track .tphase.now"), /Run 1 running · 1 todo$/);
    assert.match(await text("#track .tphase"), /Contracts 2\/2 done$/);
    assert.equal(await text("#track h2 .cnt"), "4 phases · 8 tasks · 3 gates");
    assert.equal(await text("#track h2 a.go"), "Open plan");
    assert.equal(await text('#strip [data-cell="phase"] .v'), "3 of 4 · Run");
    assert.equal(await browser.eval(`document.querySelectorAll('#strip [data-cell="phase"] .pbar i').length`), 4);
  }],

  ["hostile titles stay text, and a cycle is reported instead of hanging", async () => {
    agent.run("upsert", "-m", demo.id, "packages", "X1", "-s", "todo", `name=${EVIL}`, "phase=P4", 'depends_on=["X2"]');
    agent.run("upsert", "-m", demo.id, "packages", "X2", "-s", "todo", "name=Loop", "phase=P4", 'depends_on=["X1"]');
    await browser.goto(`${demo.url}/plan`);
    await browser.waitFor(`document.querySelectorAll("#plan .pnode").length === 10`);
    assert.match(await text("#issues .banner"), /dependency cycle X\d → X\d → X\d/);
    assert.equal(await browser.eval(`typeof window.pwned`), "undefined");
    assert.ok((await text('#plan .pnode[data-record="packages/X1"]')).includes("<img"), "the title shows as text");
  }],

  ["a flow section (v1) is a plan that draws no edges", async () => {
    agent.run("config", "patch", "-m", demo.id, JSON.stringify({ sections: [{ id: "oldflow", widget: "flow", page: "plan", lanes: { collection: "phases" }, nodes: { collection: "packages", lane: "phase" }, gates: { collection: "gates" } }] }));
    await browser.waitFor(`document.querySelectorAll("#oldflow .pnode").length === 10`);
    assert.equal(await browser.eval(`document.querySelectorAll("#oldflow .pedges path[class^=e-]").length`), 0);
    assert.equal(await browser.eval(`!!document.querySelector("#oldflow [data-plan-trace]")`), false);
  }],

  ["an old run in waves (imported state.json) still draws its plan", async () => {
    agent.write("state.json", JSON.stringify({
      run: { title: "Billing export", health: "on_track", health_reason: "wave 1" },
      waves: [{ id: "W0", title: "Contracts", state: "done" }, { id: "W1", title: "Review UI", state: "running" }],
      packages: [{ id: "R0", state: "merged", wave: "W0", name: "Contracts" }, { id: "U2", state: "in_progress", wave: "W1", name: "Review UI" }],
      gates: [{ id: "G1", state: "pending", after: "W0", title: "W0 → W1", requirements: [{ text: "R0 merged", done: true }] }],
    }));
    const imp = agent.json("import", join(agent.root, "state.json"), "-t", "multi-agent-build", "--project", agent.project);
    await browser.goto(`${imp.url}/plan`);
    await browser.waitFor(`document.querySelectorAll("#plan .pnode").length === 2`);
    const heads = await browser.eval(`[...document.querySelectorAll("#plan .phead .lbl")].map(l => l.textContent)`);
    assert.deepEqual(heads, ["Phase 1 · Contracts", "Phase 2 · Review UI · now"]);
    assert.equal(await text("#plan .pgate .mono"), "G1 1/1");
  }],

  ["gates: the next one marked, a blocked requirement shown as such, passed ones folded", async () => {
    await browser.goto(`${demo.url}/plan`);
    await browser.waitFor(`!!document.querySelector("#gates .gate.next")`);
    assert.match(await browser.eval(`document.querySelector("#gates .gate.next .gmeta").textContent`), /^G1 · 1 of 3 met · next$/);
    const reqs = await browser.eval(`[...document.querySelectorAll("#gates .gate.next .reqs li")].map(l => [l.className, l.querySelector(".mk").textContent])`);
    assert.deepEqual(reqs, [["met", "✓"], ["", "○"], ["blk", "✕"]]);
    assert.match(await browser.eval(`document.querySelector("#gates .gate.next .reqs li.blk").textContent`), /Storage adapter merged blocked/);
    assert.equal(await browser.eval(`document.querySelector("#gates details.passed summary").textContent.replace(/\\s+/g, " ").trim()`), "1 passed G0");
    assert.equal(await browser.eval(`document.querySelector("#gates details.passed").open`), false);
    await browser.eval(`document.querySelector('#gates .gate.next .reqs li.blk a').click()`);
    await browser.waitFor(`document.getElementById("drawer-title")?.textContent === "Storage adapter"`);
    await browser.key("Escape");
  }],

  ["no console errors", () => {
    assert.deepEqual(browser.errors, []);
  }],
], async () => {
  if (browser) await browser.close();
  await agent.stop();
});
