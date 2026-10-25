// The widget library in headless Chrome: every built-in renders from records, shows its empty state, and
// keeps hostile text as text; html widgets are sandboxed; script widgets load from the user's widgets/.
// Run: `just browser`, or `node tests/browser/widgets.test.mjs` after `cargo build --release`.

import assert from "node:assert/strict";
import { Agent, Browser, steps } from "./harness.mjs";

const agent = new Agent();
let browser, empty, full;
const EVIL = `<img src=x onerror="window.pwned=1"><script>window.pwned=2</script>`;

agent.write("config/widgets/heat.js", `MissionWidget.define("heat", (s, ctx) =>
  '<div class="card pad" id="heat-out">heat: ' + ctx.h.esc((ctx.records[s.collection] || []).length) + '</div>');`);
agent.write("config/config.toml", `
[widgets.heat]
script = "widgets/heat.js"
`);

const sections = `
[mission]
refresh = "poll:1"

[[sections]]
id = "c-line"
widget = "chart"
collection = "burn"
x = "at"
y = ["open", "done"]

[[sections]]
id = "c-bar"
widget = "chart"
kind = "bar"
collection = "burn"
y = "open"

[[sections]]
id = "c-spark"
widget = "chart"
kind = "sparkline"
collection = "burn"
x = "at"
y = "open"

[[sections]]
id = "sp"
widget = "spend"

[[sections]]
id = "kb"
widget = "kanban"
collection = "packages"
fields = ["wave"]

[[sections]]
id = "ck"
widget = "checklist"
collection = "gates"

[[sections]]
id = "fl"
widget = "flow"
lanes = { collection = "waves" }
nodes = { collection = "packages", lane = "wave" }
gates = { collection = "gates" }

[[sections]]
id = "md"
widget = "markdown"
record = { collection = "notes", key = "status" }

[[sections]]
id = "tb"
widget = "table"
collection = "packages"
filter = { state = ["blocked", "merged"] }

[[sections]]
id = "ht"
widget = "html"
html = "<p id='inside'>hi</p><script>try { parent.document.title = 'owned' } catch (e) { document.body.dataset.blocked = 'yes' }</script>"
height = 80

[[sections]]
id = "hs"
widget = "heat"
collection = "packages"

[[sections]]
id = "activity"
widget = "feed"
limit = 50
`;

const empties = {
  "c-line": "No data in", "c-bar": "No data in", sp: "No spend recorded yet", kb: "No packages yet",
  ck: "No gates yet", fl: "Nothing in the plan yet", md: "Nothing here yet", tb: "No packages yet",
};

await steps([
  ["two missions: one empty, one full of data", async () => {
    await agent.start();
    empty = agent.newMission("Empty", "minimal");
    agent.run("config", "patch", sections.replace('[[sections]]\nid = "ht"', '[[sections]]\nid = "ht-unused"'));
    full = agent.newMission("Full", "minimal");
    agent.run("config", "patch", sections);
    const t0 = 1792780000000;
    for (let d = 0; d < 5; d++) agent.run("upsert", "burn", `d${d}`, `at=${t0 + d * 3600000}`, `open=${10 - d}`, `done=${d}`);
    agent.run("upsert", "spend", "opus", "usd=12.5", "input_tokens=1000", "output_tokens=500");
    agent.run("upsert", "spend", "sonnet", "usd=2");
    agent.run("upsert", "waves", "W0", "-s", "done", "title=Wave 0");
    agent.run("upsert", "waves", "W1", "-s", "running", `title=${EVIL}`);
    agent.run("upsert", "packages", "R0", "-s", "merged", "name=Contracts", "wave=W0");
    agent.run("upsert", "packages", "U2", "-s", "blocked", `name=${EVIL}`, "wave=W1");
    agent.run("upsert", "packages", "U3", "-s", "in_review", "name=Docs", "wave=W1");
    agent.run("upsert", "packages", "LONG", "-s", "in_review", `name=${"x".repeat(300)}`, "wave=W1");
    agent.run("upsert", "gates", "G1", "-s", "pending", "title=W0 to W1", "after=W0",
      'requirements=[{"text":"all merged","done":true},"e2e green"]', `note=See **U2** ${EVIL}`);
    agent.run("upsert", "notes", "status", `text=# Plan\n\n- [docs](https://example.com) and [bad](javascript:alert(1))\n\n\`code\` ${EVIL}`);
    browser = await Browser.launch();
  }],

  ["every widget shows its empty state", async () => {
    await browser.goto(empty.url);
    await browser.waitFor(`document.querySelectorAll("#root section").length > 5`);
    for (const [id, text] of Object.entries(empties)) {
      const got = await browser.eval(`document.getElementById(${JSON.stringify(id)}).textContent`);
      assert.ok(got.includes(text), `${id}: ${got}`);
    }
  }],

  ["charts draw lines, bars and a sparkline at the card's width, with a table view", async () => {
    await browser.goto(full.url);
    await browser.waitFor(`!!document.querySelector("#c-line svg path")`);
    assert.equal(await browser.eval(`document.querySelectorAll("#c-line svg path[stroke]").length`), 2, "two series");
    assert.equal(await browser.eval(`document.querySelectorAll("#c-line .legend span").length`), 2);
    assert.equal(await browser.eval(`document.querySelectorAll("#c-bar svg path").length`), 5);
    assert.equal(await browser.eval(`document.querySelector("#c-spark .v").textContent`), "6");
    const w = await browser.eval(`[document.querySelector("#c-line svg").getAttribute("width") | 0, document.querySelector("#c-line .card").clientWidth]`);
    assert.ok(w[0] > 600 && w[0] <= w[1], `svg ${w[0]} in card ${w[1]}`);
    assert.equal(await browser.eval(`document.querySelectorAll("#c-line details tbody tr").length`), 5);
  }],

  ["spend totals and ranks", async () => {
    assert.match(await browser.eval(`document.getElementById("sp").textContent`), /\$14\.5/);
    assert.match(await browser.eval(`document.querySelector("#sp .bars").textContent`), /opus[\s\S]*sonnet/);
  }],

  ["kanban, checklist and flow lay records out by state, gate and lane", async () => {
    assert.equal(await browser.eval(`document.querySelectorAll("#kb .lane").length`), 3);
    assert.equal(await browser.eval(`document.querySelectorAll("#kb .kcard").length`), 4);
    assert.match(await browser.eval(`document.querySelector("#ck .gate").textContent`), /1 of 2 met/);
    assert.equal(await browser.eval(`document.querySelectorAll("#fl .phead").length`), 2, "one column per lane");
    assert.equal(await browser.eval(`document.querySelectorAll("#fl .pgate").length`), 1, "the gate sits after W0");
    assert.match(await browser.eval(`document.querySelector("#fl .phead").textContent`), /1\/1/);
    assert.equal(await browser.eval(`document.querySelectorAll("#fl .pedges path[class^=e-]").length`), 0, "flow draws no edges");
  }],

  ["hostile text stays text in every widget", async () => {
    assert.equal(await browser.eval(`typeof window.pwned`), "undefined");
    assert.equal(await browser.eval(`document.querySelectorAll("#root img, #root script").length`), 0);
    for (const id of ["kb", "fl", "ck", "md"]) {
      assert.ok((await browser.eval(`document.getElementById("${id}").textContent`)).includes("<script>"), id);
    }
  }],

  ["markdown renders a safe subset", async () => {
    assert.equal(await browser.eval(`document.querySelector("#md strong").textContent`), "Plan");
    assert.equal(await browser.eval(`document.querySelector("#md a").href`), "https://example.com/");
    assert.equal(await browser.eval(`document.querySelectorAll("#md a").length`), 1, "javascript: link not linked");
    assert.equal(await browser.eval(`document.querySelector("#md code").textContent`), "code");
  }],

  ["table filter keeps the matching rows", async () => {
    assert.deepEqual(await browser.eval(`[...document.querySelectorAll("#tb tbody tr td:first-child")].map(t => t.textContent)`), ["R0", "U2"]);
  }],

  ["html runs in a sandbox that can't touch the page", async () => {
    assert.equal(await browser.eval(`document.querySelector("#ht iframe").getAttribute("sandbox")`), "allow-scripts");
    await browser.waitFor(`(() => { try { return document.querySelector("#ht iframe").contentDocument === null } catch (e) { return true } })()`);
    await new Promise((r) => setTimeout(r, 300));
    assert.notEqual(await browser.eval(`document.title`), "owned");
  }],

  ["a script widget loads from the user's widgets/ and renders", async () => {
    await browser.waitFor(`document.getElementById("heat-out")?.textContent === "heat: 4"`);
  }],

  ["at 390 px nothing overflows the page", async () => {
    const phone = await Browser.launch({ width: 390, height: 844, mobile: true, dark: true });
    try {
      await phone.goto(full.url);
      await phone.waitFor(`!!document.querySelector("#c-line svg")`);
      assert.equal(await phone.eval(`document.documentElement.scrollWidth <= 390`), true);
      assert.deepEqual(await phone.eval(`[...document.querySelectorAll(".kcard, .gate")].filter(c => c.scrollWidth > c.clientWidth + 1).length`), 0,
        "a 300-character word wraps inside its card");
      assert.ok((await phone.eval(`document.querySelector("#c-line svg").getAttribute("width") | 0`)) <= 360);
      assert.deepEqual(phone.errors, []);
    } finally { await phone.close(); }
  }],

  ["theme overrides apply per theme, and follow the theme button", async () => {
    agent.run("config", "patch", '[theme]\naccent = "#123456"\nbg = "url(https://example.com/x.png)"\n[theme.dark]\naccent = "#654321"');
    await browser.goto(full.url);
    const accent = () => browser.eval(`getComputedStyle(document.documentElement).getPropertyValue("--accent").trim()`);
    await browser.waitFor(`getComputedStyle(document.documentElement).getPropertyValue("--accent").trim() === "#123456"`);
    await browser.eval(`document.getElementById("theme").click()`);
    assert.equal(await accent(), "#654321");
    await browser.eval(`document.getElementById("theme").click()`);
    assert.equal(await accent(), "#123456");
    assert.equal(await browser.eval(`document.documentElement.style.getPropertyValue("--bg")`), "", "a url() theme value is ignored");
  }],

  ["the feed pages through older events", async () => {
    const ops = Array.from({ length: 130 }, (_, i) => ({ op: "log", text: `event ${i}` }));
    agent.write("ops.json", JSON.stringify(ops));
    agent.run("batch", `${agent.root}/ops.json`);
    await browser.goto(`${full.url}/activity`);
    await browser.waitFor(`document.querySelectorAll("#activity .feed .row:not(.more)").length === 50`);
    const rows = () => browser.eval(`document.querySelectorAll("#activity .feed .row:not(.more)").length`);
    await browser.eval(`document.querySelector("#activity button[data-older]").click()`);
    await browser.waitFor(`document.querySelectorAll("#activity .feed .row:not(.more)").length === 100`);
    await browser.eval(`document.querySelector("#activity button[data-older]").click()`);
    await browser.waitFor(`document.querySelectorAll("#activity .feed .row:not(.more)").length > 130`);
    assert.ok((await rows()) >= 131, "older events came from the daemon");
    assert.equal(await browser.eval(`document.querySelector("#activity").textContent.includes("event 0")`), true);
  }],

  ["no console errors", () => {
    assert.deepEqual(browser.errors, []);
  }],
], async () => {
  if (browser) await browser.close();
  await agent.stop();
});
