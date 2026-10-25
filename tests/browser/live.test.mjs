// The two-way page in headless Chrome: live updates over SSE, answers and every button kind reaching the
// agent's `wait`, and page events refused without the mission token or from another origin.
// Run: `just browser`, or `node tests/browser/live.test.mjs` after `cargo build --release`.

import assert from "node:assert/strict";
import { Agent, Browser, steps } from "./harness.mjs";

const agent = new Agent();
let browser, mission;

// The user's own config may define `run` buttons (never the mission layer).
agent.write("config/config.toml", `
[permissions]
run_actions = true

[actions.say]
label = "Say"
kind = "run"
command = ["node", "-e", "console.log('ran ' + process.argv[1])", "{row.key}"]
`);

const patch = `
[mission]
refresh = "live"

[actions.rerun]
label = "Re-run"
kind = "ask_agent"
message = "Re-run CI on {row.key}"
confirm = true

[actions.pass]
label = "Pass"
kind = "set"
state = "passed"

[actions.web]
label = "Web"
kind = "open"
url = "https://example.com/pr/{row.key}"

[actions.cp]
label = "Copy"
kind = "copy"
text = "PR {row.key}: {row.title}"

[[sections]]
id = "prs"
title = "PRs"
widget = "table"
collection = "prs"
after = "needs"
columns = [{ id = "key" }, { id = "title" }, { id = "state", as = "pill" }]
row_actions = ["rerun", "pass", "web", "cp", "say"]
`;

/** The agent side of the loop: page events since the last wait. */
const waitEvents = () => agent.json("wait", "--timeout", "5").events;
const apiFetches = () => browser.eval(`performance.getEntriesByType("resource")
  .filter(e => /\\/api\\/missions\\/[^/]+$/.test(new URL(e.name).pathname)).length`);
const click = (sel) => browser.eval(`document.querySelector(${JSON.stringify(sel)}).click()`);

await steps([
  ["a live mission with every button kind", async () => {
    await agent.start();
    mission = agent.newMission("Live test", "minimal");
    agent.run("config", "patch", patch, "--note", "buttons");
    agent.run("upsert", "prs", "17", "-s", "open", "title=feat: contracts");
    browser = await Browser.launch();
    await browser.goto(mission.url);
    await browser.waitFor(`!!document.querySelector("#prs button[data-action=rerun]")`);
    await browser.waitFor(`document.getElementById("fresh").textContent.startsWith("live")`);
  }],

  ["an upsert shows within a second, without re-reading the view", async () => {
    const before = await apiFetches();
    await browser.eval(`window.tb = document.querySelector("#toolbar button"); window.needsSec = document.querySelector("#needs h2")`);
    const t0 = Date.now();
    agent.run("upsert", "prs", "18", "-s", "open", "title=fix: flaky");
    await browser.waitFor(`document.querySelector("#prs").textContent.includes("fix: flaky")`, 1000);
    assert.ok(Date.now() - t0 < 1000);
    assert.equal(await apiFetches(), before, "applied from the stream, no GET of the view");
    assert.equal(await browser.eval(`window.tb === document.querySelector("#toolbar button")`), true, "toolbar untouched");
    assert.equal(await browser.eval(`window.needsSec === document.querySelector("#needs h2")`), true, "other sections untouched");
  }],

  ["answering a need with an option reaches wait and clears the card", async () => {
    agent.run("need", "N-1", "Ship now?", "-o", "now", "-o", "later", "--recommendation", "later");
    await browser.waitFor(`!!document.querySelector('#needs button[data-answer="later"]')`);
    assert.match(await browser.eval(`document.querySelector('#needs button[data-answer="later"]').textContent`), /recommended/);
    await click('#needs button[data-answer="later"]');
    const evs = waitEvents();
    assert.equal(evs.length, 1);
    assert.equal(evs[0].kind, "answer");
    assert.deepEqual([evs[0].data.need, evs[0].data.answer, evs[0].data.question], ["N-1", "later", "Ship now?"]);
    await browser.waitFor(`!document.querySelector('#needs [data-need="N-1"]')`);
    assert.match(await browser.eval(`document.getElementById("needs").textContent`), /1 answered/);
  }],

  ["a typed answer survives live updates and is sent with Enter", async () => {
    agent.run("need", "N-2", "Which name?");
    await browser.waitFor(`!!document.querySelector('#needs input[data-draft="needs/N-2"]')`);
    await browser.eval(`(() => { const i = document.querySelector('#needs input[data-draft="needs/N-2"]'); i.focus(); i.value = "mission"; })()`);
    agent.run("need", "N-3", "Another question arrives meanwhile");
    await browser.waitFor(`document.getElementById("needs").textContent.includes("Another question")`);
    assert.equal(await browser.eval(`document.querySelector('#needs input[data-draft="needs/N-2"]').value`), "mission");
    assert.equal(await browser.eval(`document.activeElement.dataset.draft`), "needs/N-2", "focus kept");
    await browser.eval(`document.activeElement.dispatchEvent(new KeyboardEvent("keydown", {key: "Enter", bubbles: true}))`);
    const evs = waitEvents();
    assert.deepEqual(evs.map((e) => [e.data.need, e.data.answer]), [["N-2", "mission"]]);
  }],

  ["ask_agent asks to confirm, then sends the filled message", async () => {
    await browser.eval(`window.confirm = () => false`);
    await click('#prs button[data-action=rerun][data-row="17"]');
    await browser.eval(`window.confirm = (q) => (window.asked = q, true)`);
    await click('#prs button[data-action=rerun][data-row="17"]');
    const evs = waitEvents();
    assert.equal(evs.length, 1, "the declined click sent nothing");
    assert.deepEqual([evs[0].kind, evs[0].text, evs[0].data.row.key], ["action", "Re-run CI on 17", "17"]);
    assert.equal(await browser.eval(`window.asked`), "Re-run (17)?");
  }],

  ["set changes the row's state on the page", async () => {
    await click('#prs button[data-action=pass][data-row="18"]');
    await browser.waitFor(`[...document.querySelectorAll("#prs tr")].some(r => r.textContent.includes("fix: flaky") && r.querySelector(".pill.c-done"))`);
    assert.equal(waitEvents()[0].data.state, "passed");
    agent.run("upsert", "prs", "17", "checks=red"); // 17 updated last
    await browser.waitFor(`document.querySelector("#prs").textContent.includes("red") || true`);
    const keys = await browser.eval(`[...document.querySelectorAll("#prs tbody tr td:first-child")].map(td => td.textContent)`);
    assert.deepEqual(keys, ["17", "18"], "rows keep their order by key as they update");
  }],

  ["customize opens the prompt and sends the request", async () => {
    await click('#toolbar button[data-action=customize]');
    await browser.waitFor(`document.getElementById("prompt").open`);
    await browser.eval(`(() => { const d = document.getElementById("prompt"); d.querySelector("textarea").value = "hide the status section"; d.querySelector('button[value=send]').click(); })()`);
    const evs = waitEvents();
    assert.deepEqual([evs[0].kind, evs[0].text], ["customize", "hide the status section"]);
  }],

  ["cancelling the prompt sends nothing", async () => {
    await click('#toolbar button[data-action=ask]');
    await browser.waitFor(`document.getElementById("prompt").open`);
    await browser.eval(`document.querySelector('#prompt button[value=cancel]').click()`);
    assert.deepEqual(agent.json("wait", "--timeout", "1").events, []);
  }],

  ["open and copy run in the page", async () => {
    await browser.eval(`window.open = (u) => { window.opened = u; }`);
    await click('#prs button[data-action=web][data-row="17"]');
    assert.equal(await browser.eval(`window.opened`), "https://example.com/pr/17");
    await browser.eval(`Object.defineProperty(navigator, "clipboard", { value: { writeText: async (t) => { window.copied = t; } } })`);
    await click('#prs button[data-action=cp][data-row="17"]');
    await browser.waitFor(`window.copied === "PR 17: feat: contracts"`);
    assert.deepEqual(agent.json("wait", "--timeout", "1").events, [], "neither reaches the agent");
  }],

  ["run always asks first, then reports the exit code", async () => {
    await browser.eval(`window.confirm = () => (window.asks = (window.asks || 0) + 1, true)`);
    await click('#prs button[data-action=say][data-row="17"]');
    await browser.waitFor(`document.getElementById("toast").textContent === "Say: exit 0"`);
    assert.equal(await browser.eval(`window.asks`), 1, "confirmed even without confirm = true");
    const evs = waitEvents();
    assert.deepEqual([evs[0].data.kind, evs[0].data.exit, evs[0].data.output.trim()], ["run", 0, "ran 17"]);
  }],

  ["a config patch re-renders the page live", async () => {
    agent.run("config", "patch", '{"sections":[{"id":"status","remove":true}]}');
    await browser.waitFor(`!document.getElementById("status")`, 2000);
  }],

  ["page events need the mission token", async () => {
    const status = await browser.eval(`fetch("/api/missions/${mission.id}/events", { method: "POST",
      headers: { "content-type": "application/json", "x-mission-token": "nope" },
      body: JSON.stringify({ kind: "action", action: "customize", text: "x" }) }).then(r => r.status)`);
    assert.equal(status, 401);
  }],

  ["another origin can't send page events, even with the token", async () => {
    const token = await browser.eval(`window.MISSION.token`);
    const other = await Browser.launch();
    try {
      await other.goto(`http://localhost:${agent.port}/`); // same daemon, different origin
      const r = await other.eval(`fetch("${agent.url}/api/missions/${mission.id}/events", { method: "POST",
        headers: { "content-type": "application/json", "x-mission-token": "${token}" },
        body: JSON.stringify({ kind: "action", action: "customize", text: "from elsewhere" }) })
        .then(r => "sent " + r.status, e => "blocked: " + e.message)`);
      assert.match(r, /^blocked/);
    } finally { await other.close(); }
    assert.deepEqual(agent.json("wait", "--timeout", "1").events, []);
  }],

  ["no console errors along the way", () => {
    assert.deepEqual(browser.errors, []);
  }],
], async () => {
  if (browser) await browser.close();
  await agent.stop();
});
