// The record drawer on the demo mission: opens from a graph node, a table row and the URL; shows why a record
// is blocked, its relations and its history; Esc returns focus; j/k and dependency links move it; focus stays
// inside; it follows live updates; hostile text stays text; full screen on a phone.
// Run: `node tests/browser/drawer.test.mjs` after `cargo build --release`.

import assert from "node:assert/strict";
import { Agent, Browser, steps } from "./harness.mjs";
import { seedDemo } from "./demo.mjs";

const agent = new Agent();
let browser, demo;
const EVIL = `<img src=x onerror="window.pwned=1">`;
const text = (sel) => browser.eval(`document.querySelector(${JSON.stringify(sel)})?.innerText.replace(/\\s+/g, " ").trim()`);
const isOpen = () => browser.eval(`!document.getElementById("drawer").hidden`);

await steps([
  ["the demo mission", async () => {
    await agent.start();
    demo = seedDemo(agent);
    agent.run("upsert", "-m", demo.id, "packages", "S1", `branch=${EVIL}`);
    browser = await Browser.launch({ width: 1440, height: 900 });
  }],

  ["a graph node opens the drawer: why blocked, fields, relations, history", async () => {
    await browser.goto(`${demo.url}/plan`);
    await browser.waitFor(`!!document.querySelector(".pnode")`);
    assert.equal(await browser.eval(`getComputedStyle(document.getElementById("drawer")).display`), "none", "closed means not drawn");
    assert.equal(await browser.eval(`document.elementFromPoint(innerWidth - 40, 300).closest("#drawer")`), null);
    await browser.waitFor(`!!document.querySelector('.pnode[data-record="packages/S1"]')`);
    await browser.eval(`document.querySelector('.pnode[data-record="packages/S1"]').focus()`);
    await browser.key("Enter");
    await browser.waitFor(`!document.getElementById("drawer").hidden`);
    assert.equal(await text("#drawer-title"), "Storage adapter");
    assert.equal(await text("#drawer .dhead .drow .mono"), "packages / S1");
    assert.match(await text("#drawer .dhead .drow:last-child"), /^blocked Phase 2 · Build · (just now|for \d+m)$/);
    assert.equal(await text("#drawer .why"), "Why blocked: S3 credentials missing in CI");
    const deps = await browser.eval(`[...document.querySelectorAll("#drawer .dsec")].map(s => s.innerText.replace(/\\s+/g, " ").trim()).filter(t => /^(depends|holding)/i.test(t))`);
    assert.deepEqual(deps, ["DEPENDS ON C0 Contracts and schema merged", "HOLDING UP R1 Release todo G1 Build → Run 1/3 met"]);
    await browser.waitFor(`document.querySelectorAll("#drawer .hist li").length >= 2`);
    assert.match(await text("#drawer .hist li"), /^\d\d:\d\d in progress → blocked$/);
    assert.equal(await browser.eval(`location.search`), "?record=packages%2FS1");
    assert.equal(await browser.eval(`document.getElementById("drawer").contains(document.activeElement)`), true, "focus moved in");
  }],

  ["hostile text in a field stays text", async () => {
    assert.ok((await text("#drawer .kv")).includes("<img src=x"));
    assert.equal(await browser.eval(`typeof window.pwned`), "undefined");
  }],

  ["focus stays inside while it's open", async () => {
    for (let i = 0; i < 25; i++) {
      await browser.key("Tab");
      assert.equal(await browser.eval(`document.getElementById("drawer").contains(document.activeElement)`), true, `tab ${i}`);
    }
  }],

  ["Esc closes it and gives focus back to the node", async () => {
    await browser.key("Escape");
    await browser.waitFor(`document.getElementById("drawer").hidden`);
    assert.equal(await browser.eval(`getComputedStyle(document.getElementById("drawer")).display`), "none");
    assert.equal(await browser.eval(`document.activeElement.dataset.record`), "packages/S1");
    assert.equal(await browser.eval(`location.search`), "");
  }],

  ["j and k walk the widget's records; a dependency link moves the drawer there", async () => {
    await browser.eval(`document.querySelector('.pnode[data-record="packages/S1"]').click()`);
    await browser.waitFor(`document.getElementById("drawer-title")?.textContent === "Storage adapter"`);
    await browser.eval(`document.querySelector("#drawer [data-dclose]").focus()`);
    await browser.key("j");
    await browser.waitFor(`document.getElementById("drawer-title").textContent === "Export screen"`);
    await browser.key("k");
    await browser.waitFor(`document.getElementById("drawer-title").textContent === "Storage adapter"`);
    await browser.eval(`document.querySelector('#drawer .deps a[data-record="packages/C0"]').click()`);
    await browser.waitFor(`document.getElementById("drawer-title").textContent === "Contracts and schema"`);
    await browser.key("Escape");
  }],

  ["the URL reopens it, and it follows live updates", async () => {
    await browser.goto(`${demo.url}/plan?record=packages/S1`);
    await browser.waitFor(`document.getElementById("drawer-title")?.textContent === "Storage adapter"`);
    agent.run("upsert", "-m", demo.id, "packages", "S1", "-s", "in_progress", "reason=null");
    await browser.waitFor(`document.querySelector("#drawer .dhead .pill")?.textContent === "in progress"`);
    await browser.waitFor(`document.querySelector("#drawer .hist li")?.textContent.includes("blocked → in progress")`);
    assert.equal(await browser.eval(`!!document.querySelector("#drawer .why")`), false);
    await browser.key("Escape");
  }],

  ["a table row opens it too (click or Enter), and Esc returns focus to the row", async () => {
    await browser.goto(demo.url);
    await browser.waitFor(`!!document.querySelector('#inflight tr[data-key="U1"]')`);
    await browser.eval(`document.querySelector('#inflight tr[data-key="U1"]').focus()`);
    await browser.key("Enter");
    await browser.waitFor(`document.getElementById("drawer-title")?.textContent === "Export screen"`);
    assert.match(await text("#drawer .deps"), /N-3 Should exports include archived invoices\? needs you/);
    await browser.key("Escape");
    await browser.waitFor(`document.getElementById("drawer").hidden`);
    assert.equal(await browser.eval(`document.activeElement.dataset.key`), "U1");
    await browser.eval(`document.querySelector('#inflight tr[data-key="J1"] td:nth-child(3)').click()`);
    assert.equal(await isOpen(), true);
    assert.equal(await text("#drawer-title"), "Nightly job");
    await browser.key("Escape");
  }],

  ["on a phone it's full screen with a back button", async () => {
    const phone = await Browser.launch({ width: 390, height: 844, mobile: true });
    try {
      await phone.goto(`${demo.url}/plan?record=packages/R1`);
      await phone.waitFor(`!document.getElementById("drawer").hidden`);
      assert.equal(await phone.eval(`document.getElementById("drawer").getBoundingClientRect().width`), 390);
      assert.equal(await phone.eval(`getComputedStyle(document.querySelector("#drawer .dclose .back")).display !== "none"`), true);
      assert.deepEqual(phone.errors, []);
    } finally { await phone.close(); }
  }],

  ["reduced motion: the drawer opens without sliding; forced colors: bars and blocks stay visible", async () => {
    agent.run("upsert", "-m", demo.id, "packages", "S1", "-s", "blocked");
    const b = await Browser.launch({ media: [{ name: "prefers-reduced-motion", value: "reduce" }, { name: "forced-colors", value: "active" }] });
    try {
      await b.goto(`${demo.url}/plan?record=packages/U1`);
      await b.waitFor(`!document.getElementById("drawer").hidden`);
      assert.equal(await b.eval(`getComputedStyle(document.getElementById("drawer")).animationName`), "none");
      assert.equal(await b.eval(`getComputedStyle(document.querySelector("#plan .mixbar i")).borderTopStyle`), "solid", "mix bar segments are outlined");
      assert.equal(await b.eval(`getComputedStyle(document.querySelector("#plan .pnode.blk")).borderTopWidth`), "2px");
      assert.equal(await b.eval(`getComputedStyle(document.querySelector(".pill")).borderTopStyle`), "solid", "pills keep a border");
      assert.deepEqual(b.errors, []);
    } finally { await b.close(); }
  }],

  ["no console errors", () => {
    assert.deepEqual(browser.errors, []);
  }],
], async () => {
  if (browser) await browser.close();
  await agent.stop();
});
