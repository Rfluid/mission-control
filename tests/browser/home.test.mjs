// The missions home: empty, then the demo missions in different states (on track with a question, blocked,
// at risk, finished): needs across missions, cards with health, progress and stats, the filter, the finished
// table, ⌘K over missions; one request draws it. Run: `node tests/browser/home.test.mjs` after a release build.

import assert from "node:assert/strict";
import { Agent, Browser, steps } from "./harness.mjs";
import { seedDemo, seedHome } from "./demo.mjs";

const agent = new Agent();
let browser;
const text = (sel) => browser.eval(`document.querySelector(${JSON.stringify(sel)})?.innerText.replace(/\\s+/g, " ").trim()`);
const cards = () => browser.eval(`[...document.querySelectorAll(".mcard .mt")].map(t => t.textContent)`);

await steps([
  ["with no missions, the home says how to start one", async () => {
    await agent.start();
    browser = await Browser.launch({ width: 1440, height: 900 });
    await browser.goto(`${agent.url}/`);
    await browser.waitFor(`document.getElementById("missions").textContent.includes("No missions yet")`);
    assert.equal(await browser.eval(`document.getElementById("needs").hidden`), true);
    assert.match(await text("#addr"), /^127\.0\.0\.1:\d+ · v\d+\.\d+\.\d+$/);
  }],

  ["missions in different states, drawn from one request", async () => {
    seedDemo(agent);
    seedHome(agent);
    agent.run("new", "<img src=x onerror=window.pwned=1>", "--project", agent.project);
    await browser.goto(`${agent.url}/`);
    await browser.waitFor(`document.querySelectorAll(".mcard").length === 4`);
    const api = await browser.eval(`performance.getEntriesByType("resource").filter(e => new URL(e.name).pathname.startsWith("/api/")).map(e => new URL(e.name).pathname)`);
    assert.deepEqual(api, ["/api/missions"]);
    assert.equal(await browser.eval(`typeof window.pwned`), "undefined");
  }],

  ["needs you across missions, with what each blocks", async () => {
    assert.match(await text("#needs h2"), /^NEEDS YOU 2 across 2 missions$/);
    const rows = await browser.eval(`[...document.querySelectorAll("#needs .nrow")].map(r => r.innerText.replace(/\\s+/g, " ").trim())`);
    assert.ok(rows.some((r) => /^Billing export build Should exports include archived invoices\? · blocks U1/.test(r)), rows.join("\n"));
    assert.ok(rows.some((r) => /^PR babysit: auth-v2 Rebase onto main or merge main in\? · blocks 512/.test(r)), rows.join("\n"));
  }],

  ["a card: health and reason, progress from the plan, stats from the strip", async () => {
    const card = `[...document.querySelectorAll(".mcard")].find(c => c.querySelector(".mt").textContent === "Billing export build")`;
    const t = await browser.eval(`${card}.innerText.replace(/\\s+/g, " ")`);
    assert.match(t, /1 needs you/);
    assert.match(t, /on track Build phase merging; storage adapter blocked on credentials\./);
    assert.match(t, /Phase 2 of 4 · Build/);
    assert.match(t, /Tasks 3\/8 done Blocked 1 Agents 2 running/);
    assert.deepEqual(await browser.eval(`[...${card}.querySelectorAll(".segs i")].map(i => i.className)`), ["sg-done", "sg-now", "sg-ahead", "sg-ahead"]);
    const babysit = await browser.eval(`[...document.querySelectorAll(".mcard")].find(c => c.querySelector(".mt").textContent.startsWith("PR babysit")).innerText.replace(/\\s+/g, " ")`);
    assert.match(babysit, /blocked CI red on #512 after main moved/);
    assert.match(babysit, /2 of 3 prs done/);
  }],

  ["the filter: Active, Needs you, Finished; the choice is kept", async () => {
    await browser.eval(`document.querySelector('[data-filter="needs"]').click()`);
    assert.deepEqual((await cards()).sort(), ["Billing export build", "PR babysit: auth-v2"]);
    await browser.eval(`document.querySelector('[data-filter="finished"]').click()`);
    assert.equal((await cards()).length, 3);
    await browser.goto(`${agent.url}/`);
    await browser.waitFor(`document.querySelectorAll(".mcard").length === 3`);
    assert.equal(await browser.eval(`document.querySelector('[data-filter="finished"]').getAttribute("aria-pressed")`), "true");
    await browser.eval(`document.querySelector('[data-filter="active"]').click()`);
  }],

  ["recently finished: mission, template, result, ended", async () => {
    const rows = await browser.eval(`[...document.querySelectorAll("#finished tbody tr")].map(r => [...r.cells].slice(0, 3).map(c => c.innerText.trim()))`);
    assert.equal(rows.length, 3);
    assert.ok(rows.some(([m, t, r]) => m === "Search relevance spike" && t === "research" && r === "closed"), JSON.stringify(rows));
  }],

  ["⌘K finds a mission and opens it", async () => {
    await browser.key("k", 2);
    await browser.waitFor(`document.getElementById("palette").open`);
    for (const c of "flaky") await browser.key(c);
    await browser.waitFor(`document.querySelector('#palette .pal-i[aria-selected="true"] .pal-l')?.textContent === "Flaky test hunt"`);
    await browser.key("Enter");
    await browser.waitFor(`location.pathname.startsWith("/m/msn_")`);
    assert.equal(await browser.eval(`document.querySelector("h1.brand").textContent`), "Flaky test hunt");
  }],

  ["at 390 px nothing overflows", async () => {
    const phone = await Browser.launch({ width: 390, height: 844, mobile: true });
    try {
      await phone.goto(`${agent.url}/`);
      await phone.waitFor(`document.querySelectorAll(".mcard").length >= 3`);
      assert.equal(await phone.eval(`document.documentElement.scrollWidth <= innerWidth`), true);
      assert.deepEqual(phone.errors, []);
    } finally { await phone.close(); }
  }],

  ["no console errors", () => {
    assert.deepEqual(browser.errors, []);
  }],
], async () => {
  if (browser) await browser.close();
  await agent.stop();
});
