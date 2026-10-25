// Attention signals on the demo mission: the title counts open needs, the favicon follows health and needs,
// the bell opts this browser in, and a notification fires once per new need or new block, never on progress.
// Run: `node tests/browser/attention.test.mjs` after `cargo build --release`.

import assert from "node:assert/strict";
import { Agent, Browser, steps } from "./harness.mjs";
import { seedDemo } from "./demo.mjs";

const agent = new Agent();
let browser, demo;
const icon = () => browser.eval(`(() => { const l = document.querySelector('link[rel="icon"]'); return [l.href.slice(0, 22), l.dataset.tone, l.dataset.ring]; })()`);
/** A Notification stand-in that records what would show; the page believes it's in the background. */
const STUB = `(() => {
  window.__notes = [];
  window.Notification = class { static permission = "default"; static async requestPermission() { this.permission = "granted"; return "granted"; }
    constructor(title, o) { window.__notes.push([title, o.body, o.tag]); } close() {} };
  document.hasFocus = () => false;
})()`;
const notes = () => browser.eval(`window.__notes`);
const up = (...a) => agent.run("upsert", "-m", demo.id, ...a);

await steps([
  ["the demo mission", async () => {
    await agent.start();
    demo = seedDemo(agent);
    browser = await Browser.launch();
    await browser.goto(demo.url);
    await browser.waitFor(`!!document.querySelector("#needs .need")`);
  }],

  ["the title counts open needs; the favicon is the beacon in the health tone with a needs ring", async () => {
    assert.equal(await browser.eval(`document.title`), "(1) Billing export build · mission-control");
    assert.deepEqual(await icon(), ["data:image/png;base64,", "done", "true"]);
  }],

  ["the bell asks once and turns this browser's notifications on", async () => {
    await browser.eval(STUB);
    assert.equal(await browser.eval(`document.getElementById("bell").getAttribute("aria-pressed")`), "false");
    await browser.eval(`document.getElementById("bell").click()`);
    await browser.waitFor(`document.getElementById("bell").getAttribute("aria-pressed") === "true"`);
    assert.equal(await browser.eval(`localStorage.getItem("mission-control.notify")`), "on");
  }],

  ["a new need: one notification and a polite announcement; the title and ring follow", async () => {
    agent.run("need", "-m", demo.id, "N-4", "Retry the nightly job now?", "-o", "yes", "-o", "no", "--blocks", "J1");
    await browser.waitFor(`document.title.startsWith("(2) ")`);
    assert.deepEqual(await notes(), [["Question: Retry the nightly job now?", "Billing export build", "need-N-4"]]);
    assert.equal(await browser.eval(`document.getElementById("announce").textContent`), "New question: Retry the nightly job now?");
    up("needs", "N-4", "context=More detail");
    await new Promise((r) => setTimeout(r, 700));
    assert.equal((await notes()).length, 1, "an update to an open need doesn't notify again");
  }],

  ["progress never notifies; a new block does, once", async () => {
    up("packages", "D1", "-s", "in_progress");
    up("packages", "J1", "-s", "in_review");
    await new Promise((r) => setTimeout(r, 700));
    assert.equal((await notes()).length, 1, "progress is quiet");
    up("packages", "J1", "-s", "blocked", "reason=Runner out of disk");
    await browser.waitFor(`window.__notes.length === 2`);
    assert.deepEqual((await notes())[1], ["J1 Nightly job is blocked", "Runner out of disk", "block-packages-J1"]);
    assert.equal(await browser.eval(`document.getElementById("announce").textContent`), "J1 is blocked: Runner out of disk");
    up("packages", "J1", "note=still");
    await new Promise((r) => setTimeout(r, 700));
    assert.equal((await notes()).length, 2);
  }],

  ["answering clears the count and the ring; health drives the signal's tone", async () => {
    up("needs", "N-3", "-s", "answered", "answer=Leave them out");
    up("needs", "N-4", "-s", "answered", "answer=no");
    await browser.waitFor(`document.title === "Billing export build · mission-control"`);
    assert.deepEqual(await icon(), ["data:image/png;base64,", "done", "false"]);
    up("run", "run", "-s", "at_risk");
    await browser.waitFor(`document.querySelector('link[rel="icon"]').dataset.tone === "warn"`);
  }],

  ["no console errors", () => {
    assert.deepEqual(browser.errors, []);
  }],
], async () => {
  if (browser) await browser.close();
  await agent.stop();
});
