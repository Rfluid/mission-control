// The Architecture page on the demo mission: the `graph` widget (groups, edges, task state, blocked edges, the
// drawer), the `doc` widget with a Mermaid diagram drawn in the page's tokens, a bad block that shows its
// source and error, and hostile diagram text that runs nothing. The renderer loads only where it's needed.
// Run: `node tests/browser/architecture.test.mjs` after `cargo build --release`.

import assert from "node:assert/strict";
import { join } from "node:path";
import { Agent, Browser, steps } from "./harness.mjs";
import { seedDemo } from "./demo.mjs";

const agent = new Agent();
let browser, demo;
const text = (sel) => browser.eval(`document.querySelector(${JSON.stringify(sel)})?.innerText.replace(/\\s+/g, " ").trim()`);
const mermaidLoads = () => browser.eval(`performance.getEntriesByType("resource").filter(e => e.name.includes("/assets/vendor/mermaid")).length`);

await steps([
  ["the demo mission", async () => {
    await agent.start();
    demo = seedDemo(agent);
    browser = await Browser.launch({ width: 1440, height: 900 });
  }],

  ["the renderer isn't loaded on a page with no diagram", async () => {
    await browser.goto(demo.url);
    await browser.waitFor(`!!document.querySelector("#needs .need")`);
    assert.equal(await mermaidLoads(), 0);
  }],

  ["graph: groups as bands, components as boxes, live task state, labelled edges", async () => {
    await browser.goto(`${demo.url}/architecture`);
    await browser.waitFor(`document.querySelectorAll("#system .gnode").length === 8`);
    assert.deepEqual(await browser.eval(`[...document.querySelectorAll("#system .glabel")].map(l => l.textContent)`), ["Browser", "API", "Workers", "Storage"]);
    assert.equal(await browser.eval(`document.querySelectorAll("#system .pedges > path").length`), 6);
    assert.equal(await browser.eval(`document.querySelectorAll("#system .pedges > path.e-block").length`), 1, "job → S3 bucket: into a blocked node");
    const s3 = '#system .gnode[data-record="packages/S1"]';
    assert.match(await text(s3), /S3 bucket S1 blocked/);
    assert.equal(await browser.eval(`document.querySelector(${JSON.stringify(s3)}).classList.contains("blk")`), true);
    assert.deepEqual(await browser.eval(`[...document.querySelectorAll("#system .glab")].map(l => l.textContent).sort()`), ["HTTP", "enqueue", "pulls", "reads", "session", "writes CSV"]);
    assert.equal(await text("#system h2 .cnt"), "8 components");
    await browser.eval(`document.querySelector(${JSON.stringify(s3)}).click()`);
    await browser.waitFor(`document.getElementById("drawer-title")?.textContent === "Storage adapter"`);
    await browser.key("Escape");
  }],

  ["doc: headings, prose, and a Mermaid diagram drawn with the page's tokens", async () => {
    await browser.waitFor(`!!document.querySelector("#howitworks figure.mmd svg")`, 20000);
    assert.deepEqual(await browser.eval(`[...document.querySelectorAll("#howitworks h3")].map(h => h.textContent)`), ["Request path", "The nightly job", "Open questions"]);
    assert.equal(await browser.eval(`document.querySelectorAll("#howitworks figure.mmd script, #howitworks figure.mmd foreignObject").length`), 0);
    assert.match(await browser.eval(`document.querySelector("#howitworks figure.mmd svg").textContent`), /POST \/exports/);
    assert.equal(await mermaidLoads(), 1);
    const r = await fetch(`${agent.url}/assets/vendor/mermaid-11.12.3.js`, { headers: { "accept-encoding": "gzip" } });
    assert.equal(r.headers.get("content-type"), "text/javascript");
  }],

  ["a bad block shows its source and the error, and the rest of the doc still renders", async () => {
    const body = "### Broken\n\n```mermaid\nsequenceDiagram\n  A->>\n```\n\nStill here.";
    agent.write("bad.json", JSON.stringify([{ op: "upsert", collection: "docs", key: "architecture", patch: { body } }]));
    agent.run("batch", "-m", demo.id, join(agent.root, "bad.json"));
    await browser.waitFor(`!!document.querySelector("#howitworks figure.mmd.bad .mmd-err")`, 20000);
    assert.match(await text("#howitworks figure.mmd.bad pre"), /sequenceDiagram A->>/);
    assert.match(await text("#howitworks .mmd-err"), /^Diagram error:/);
    assert.match(await text("#howitworks"), /Still here\./);
  }],

  ["hostile diagram text runs nothing", async () => {
    const body = "```mermaid\nflowchart LR\n  A[\"<img src=x onerror=window.pwned=1>\"] --> B\n  click A call alert(1)\n  B --> C[javascript:alert(1)]\n```";
    agent.write("evil.json", JSON.stringify([{ op: "upsert", collection: "docs", key: "architecture", patch: { body } }]));
    agent.run("batch", "-m", demo.id, join(agent.root, "evil.json"));
    await browser.waitFor(`!!document.querySelector("#howitworks figure.mmd[data-theme]")`, 20000);
    await new Promise((r) => setTimeout(r, 500));
    assert.equal(await browser.eval(`typeof window.pwned`), "undefined");
    assert.equal(await browser.eval(`document.querySelectorAll("#howitworks img, #howitworks [onclick], #howitworks a").length`), 0);
    assert.deepEqual(browser.dialogs, []);
  }],

  ["a fresh mission's Architecture page says what fills it", async () => {
    const fresh = agent.newMission("Empty build", "multi-agent-build");
    await browser.goto(`${fresh.url}/architecture`);
    await browser.waitFor(`!!document.querySelector("#system .empty")`);
    assert.equal(await text("#system .empty"), "No components yet.");
    assert.equal(await text("#howitworks .empty"), "Nothing written yet.");
  }],

  ["no console errors", () => {
    assert.deepEqual(browser.errors.filter((e) => !/Syntax error|Parse error|Expecting/i.test(e)), []);
  }],
], async () => {
  if (browser) await browser.close();
  await agent.stop();
});
