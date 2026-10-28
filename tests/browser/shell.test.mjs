// The shell on the demo mission (tests/browser/demo.mjs): status strip, tabs, compact strip, live indicator,
// and principle 1: health, the open-needs count and the first need show without scrolling.
// Run: `just browser`, or `node tests/browser/shell.test.mjs` after `cargo build --release`.

import assert from "node:assert/strict";
import { Agent, Browser, steps } from "./harness.mjs";
import { seedDemo } from "./demo.mjs";

const agent = new Agent();
let browser, demo, quiet;

/** Bottom edges (CSS px) of the health cell, the needs cell and the first need's question. */
const fold = (b) => b.eval(`(() => {
  const r = s => { const e = document.querySelector(s); return e ? Math.round(e.getBoundingClientRect().bottom) : null; };
  return { vh: innerHeight, health: r("#strip .cell.hl"), needs: r("#strip .cell.needs"), need: r("#needs .need h3") };
})()`);

await steps([
  ["the demo mission and a quiet one", async () => {
    await agent.start();
    demo = seedDemo(agent);
    quiet = agent.newMission("Quiet", "minimal");
    browser = await Browser.launch({ width: 1280, height: 800 });
  }],

  ["the full strip on Overview: health, cells, needs", async () => {
    await browser.goto(demo.url);
    await browser.waitFor(`!!document.querySelector("#strip .cell.needs")`);
    assert.match(await browser.eval(`document.querySelector("#strip .cell.hl").textContent`), /on track.*storage adapter blocked/);
    const cells = await browser.eval(`[...document.querySelectorAll("#strip [data-cell]")].map(c => [c.dataset.cell, c.querySelector(".v").textContent])`);
    assert.deepEqual(cells, [["phase", "3 of 4 · Run"], ["tasks", "3/8 done"], ["blocked", "1"], ["agents", "2 running"], ["spend", "$14.20"]]);
    assert.equal(await browser.eval(`document.querySelector('[data-cell="blocked"] .v').classList.contains("t-block")`), true);
    const needs = await browser.eval(`(() => { const a = document.querySelector("#strip a.cell.needs.on"); return [a.textContent, new URL(a.href).hash, !!a.querySelector("svg.g")]; })()`);
    assert.deepEqual(needs, ["Needs you1 question", "#needs", true]);
    assert.equal(await browser.eval(`document.getElementById("strip").hidden`), false);
  }],

  ["at 0 needs the strip says nothing waits, with no fill", async () => {
    await browser.goto(quiet.url);
    await browser.waitFor(`!!document.querySelector("#strip .cell.needs")`);
    assert.equal(await browser.eval(`document.querySelector("#strip .cell.needs").textContent`), "Needs youNothing waits on you");
    assert.equal(await browser.eval(`document.querySelector("#strip .cell.needs").classList.contains("on")`), false);
    assert.equal(await browser.eval(`document.querySelectorAll("#strip [data-cell]").length`), 0);
  }],

  ["tabs: config order, counts, sticky; other pages show the compact strip", async () => {
    await browser.goto(demo.url);
    await browser.waitFor(`document.querySelectorAll("#nav a.tab").length > 1`);
    const tabs = await browser.eval(`[...document.querySelectorAll("#nav a.tab")].map(a => a.textContent.trim().replace(/\\s+/g, " "))`);
    assert.deepEqual(tabs, ["Overview", "Plan 8", "Architecture", "Pull requests 1", "Tests", "Agents 2", "Decisions 4", "Activity"]);
    assert.equal(await browser.eval(`getComputedStyle(document.querySelector("nav.tabs")).position`), "sticky");
    await browser.eval(`document.querySelector('#nav a[data-page="prs"]').click()`);
    await browser.waitFor(`document.getElementById("strip").hidden`);
    const compact = await browser.eval(`[...document.querySelectorAll("#compact .pill")].map(p => p.textContent)`);
    assert.deepEqual(compact, ["on track", "1 needs you"]);
    assert.match(await browser.eval(`document.querySelector("#compact a").getAttribute("href")`), /\/m\/msn_[0-9a-f]+#needs$/);
  }],

  ["Overview has two columns; the rail holds numbers, the burndown and recent activity", async () => {
    await browser.goto(demo.url);
    await browser.waitFor(`!!document.querySelector(".col-rail #overview")`);
    const rail = await browser.eval(`[...document.querySelectorAll(".col-rail section")].map(s => s.id)`);
    assert.deepEqual(rail, ["overview", "burn", "recent"]);
    assert.equal(await browser.eval(`document.querySelector(".col-main section").id`), "needs", "Needs you first in main");
    const kpi = await browser.eval(`document.querySelector("#overview .kpi").textContent`);
    assert.match(kpi, /Tasks done\s*3 \/ 8/);
    assert.equal(await browser.eval(`document.querySelectorAll("#recent .feed .row").length`), 7);
    assert.equal(await browser.eval(`document.querySelector("#recent h2 a.go").textContent`), "All");
  }],

  ["principle 1 at 1280 × 800: health, needs count and the first need above the fold", async () => {
    const f = await fold(browser);
    for (const k of ["health", "needs", "need"]) assert.ok(f[k] != null && f[k] <= f.vh, `${k} ${f[k]} > ${f.vh}`);
  }],

  ["principle 1 at 390 × 844", async () => {
    const phone = await Browser.launch({ width: 390, height: 844, mobile: true });
    try {
      await phone.goto(demo.url);
      await phone.waitFor(`!!document.querySelector("#needs .need h3")`);
      const f = await fold(phone);
      for (const k of ["health", "needs", "need"]) assert.ok(f[k] != null && f[k] <= f.vh, `${k} ${f[k]} > ${f.vh}`);
      assert.equal(await phone.eval(`document.documentElement.scrollWidth <= innerWidth`), true, "no sideways scroll");
      assert.deepEqual(phone.errors, []);
    } finally { await phone.close(); }
  }],

  ["a need card: what it blocks, the recommendation in ink, numbered options, answered ones folded", async () => {
    await browser.goto(demo.url);
    await browser.waitFor(`!!document.querySelector("#needs .need")`);
    const meta = await browser.eval(`document.querySelector("#needs .need .meta").textContent`);
    assert.match(meta, /^N-3 · .+ · blocks U1 Export screen$/);
    assert.equal(await browser.eval(`new URLSearchParams(document.querySelector("#needs .need .meta a").search).get("record")`), "packages/U1");
    const opts = await browser.eval(`[...document.querySelectorAll("#needs .opts button")].map(b => [b.querySelector(".kbd").textContent, b.classList.contains("rec")])`);
    assert.deepEqual(opts, [["1", true], ["2", false], ["3", false]]);
    assert.equal(await browser.eval(`getComputedStyle(document.querySelector("#needs .need")).borderTopColor === getComputedStyle(document.documentElement).getPropertyValue("--warn").trim().replace(/^#(..)(..)(..)$/, (_, r, g, b) => "rgb(" + [r, g, b].map(x => parseInt(x, 16)).join(", ") + ")")`), true);
    assert.equal(await browser.eval(`document.querySelector("#needs h2 .cnt").textContent`), "1 open · 2 answered");
    assert.equal(await browser.eval(`document.querySelector("#needs details.answered summary").textContent`), "2 answered");
  }],

  ["answering a need with the keyboard only", async () => {
    // Tab from the top of the page to the first option, then press 2.
    await browser.eval(`document.activeElement.blur()`);
    for (let i = 0; i < 40 && !(await browser.eval(`!!document.activeElement.closest(".need .opts")`)); i++) await browser.key("Tab");
    assert.equal(await browser.eval(`document.activeElement.dataset.answer`), "Include them, flagged", "focus reached the first option");
    assert.equal(await browser.eval(`getComputedStyle(document.activeElement).outlineStyle`), "solid", "focus is visible");
    await browser.key("2");
    await browser.waitFor(`!document.querySelector("#needs .need")`);
    const answer = agent.json("wait", "-m", demo.id, "--timeout", "5").events.find((e) => e.kind === "answer");
    assert.equal(answer.data.answer, "Leave them out");
    assert.equal(await browser.eval(`document.querySelector("#strip .cell.needs").textContent`), "Needs youNothing waits on you");
  }],

  ["the live indicator says live, then reconnecting when the daemon goes away", async () => {
    await browser.goto(demo.url);
    await browser.waitFor(`document.getElementById("fresh").dataset.state === "live"`);
    assert.match(await browser.eval(`document.getElementById("fresh").textContent`), /^live · \d+s ago$/);
    const exited = new Promise((r) => agent.daemon.once("exit", r));
    agent.daemon.kill();
    await exited;
    await browser.waitFor(`document.getElementById("fresh").dataset.state === "reconnecting"`);
    assert.equal(await browser.eval(`document.getElementById("fresh").textContent`), "reconnecting");
    browser.errors.length = 0; // the dropped stream logs network errors; that's the point of this step
  }],
], async () => {
  if (browser) await browser.close();
  await agent.stop();
});
