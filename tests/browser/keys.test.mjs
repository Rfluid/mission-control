// The keyboard on the demo mission: the palette (Ctrl+K, `/`) drives a page change, a drawer and an action
// with no mouse; `g` jumps, `n` finds the question, `?` lists the keys; nothing fires while typing; and
// `[ui] shortcuts = false` turns single keys off while Ctrl+K keeps working.
// Run: `node tests/browser/keys.test.mjs` after `cargo build --release`.

import assert from "node:assert/strict";
import { Agent, Browser, steps } from "./harness.mjs";
import { seedDemo } from "./demo.mjs";

const agent = new Agent();
let browser, demo;
const CTRL = 2;
/** Types text into whatever has focus, one key at a time. */
const type = async (s) => { for (const c of s) await browser.key(c); };
const path = () => browser.eval(`location.pathname.replace(/^\\/m\\/[^/]+/, "") || "/"`);
const paletteOpen = () => browser.eval(`document.getElementById("palette").open`);

await steps([
  ["the demo mission", async () => {
    await agent.start();
    demo = seedDemo(agent);
    browser = await Browser.launch({ width: 1280, height: 900 });
    await browser.goto(demo.url);
    await browser.waitFor(`!!document.querySelector("#needs .need")`);
    await browser.eval(`document.activeElement.blur()`);
  }],

  ["Ctrl+K opens the palette; typing and Enter go to a page", async () => {
    await browser.key("k", CTRL);
    await browser.waitFor(`document.getElementById("palette").open`);
    assert.equal(await browser.eval(`document.activeElement.closest("#palette") !== null`), true, "focus in the search box");
    await type("plan");
    await browser.waitFor(`document.querySelector('#palette .pal-i[aria-selected="true"] .pal-l')?.textContent === "Plan"`);
    await browser.key("Enter");
    await browser.waitFor(`location.pathname.endsWith("/plan")`);
    assert.equal(await paletteOpen(), false);
  }],

  ["/ then a record key opens its drawer; Esc closes it", async () => {
    await browser.key("/");
    await browser.waitFor(`document.getElementById("palette").open`);
    await type("S1 storage");
    await browser.waitFor(`document.querySelector('#palette .pal-i[aria-selected="true"] .pal-l')?.textContent === "S1 Storage adapter"`);
    assert.equal(await browser.eval(`document.querySelector('#palette .pal-i[aria-selected="true"]').previousElementSibling.textContent`), "Records");
    await browser.key("Enter");
    await browser.waitFor(`document.getElementById("drawer-title")?.textContent === "Storage adapter"`);
    await browser.key("Escape");
    await browser.waitFor(`document.getElementById("drawer").hidden`);
  }],

  ["an action from the keyboard alone reaches the agent", async () => {
    await browser.key("k", CTRL);
    await browser.waitFor(`document.getElementById("palette").open`);
    await type("ask agent");
    await browser.waitFor(`document.querySelector('#palette .pal-i[aria-selected="true"]')?.previousElementSibling?.textContent === "Actions"`);
    await browser.key("Enter");
    await browser.waitFor(`document.getElementById("prompt").open`);
    await type("status?");
    await browser.key("Tab");
    await browser.key("Tab");
    assert.equal(await browser.eval(`document.activeElement.value`), "send");
    await browser.key("Enter");
    await browser.waitFor(`!document.getElementById("prompt").open`);
    const ev = agent.json("wait", "-m", demo.id, "--timeout", "5").events.find((e) => e.kind === "action");
    assert.equal(ev.data.text, "status?");
  }],

  ["g then a letter jumps; n focuses the open question", async () => {
    await browser.eval(`document.activeElement.blur()`);
    await browser.key("g"); await browser.key("o");
    await browser.waitFor(`!/\\/(plan|activity)$/.test(location.pathname)`);
    assert.equal(await path(), "/");
    await browser.key("g"); await browser.key("a");
    await browser.waitFor(`location.pathname.endsWith("/activity")`);
    await browser.key("n");
    await browser.waitFor(`document.activeElement?.dataset?.needCard === "N-3"`);
    assert.equal(await path(), "/");
  }],

  ["? lists the keys, with each page's letter; Esc closes it", async () => {
    await browser.eval(`document.activeElement.blur()`);
    await browser.key("?");
    await browser.waitFor(`document.getElementById("keys").open`);
    const sheet = await browser.eval(`document.querySelector("#keys .keys").innerText`);
    for (const s of ["Command palette", "Go to Plan", "Go to Architecture", "Go to Activity", "First open question"]) assert.ok(sheet.includes(s), s);
    await browser.key("Escape");
    await browser.waitFor(`!document.getElementById("keys").open`);
  }],

  ["nothing fires while typing in a text box", async () => {
    await browser.eval(`document.querySelector('#needs input[data-draft]').focus()`);
    await type("g/p?n1");
    assert.equal(await browser.eval(`document.querySelector('#needs input[data-draft]').value`), "g/p?n1");
    assert.equal(await paletteOpen(), false);
    assert.equal(await browser.eval(`document.getElementById("keys").open`), false);
    assert.equal(await path(), "/");
    assert.equal(await browser.eval(`!!document.querySelector("#needs .need")`), true, "1 didn't answer the question");
    await browser.eval(`document.querySelector('#needs input[data-draft]').value = ""`);
  }],

  ["[ui] shortcuts = false turns single keys off; Ctrl+K still opens the palette", async () => {
    agent.run("config", "patch", "-m", demo.id, '{"ui":{"shortcuts":false}}');
    await browser.goto(demo.url);
    await browser.waitFor(`!!document.querySelector("#needs .need")`);
    await browser.eval(`document.activeElement.blur()`);
    await browser.key("/");
    await browser.key("g"); await browser.key("p");
    assert.equal(await paletteOpen(), false);
    assert.equal(await path(), "/");
    await browser.key("k", CTRL);
    await browser.waitFor(`document.getElementById("palette").open`);
    await browser.key("Escape");
    await browser.waitFor(`!document.getElementById("palette").open`);
  }],

  ["no console errors", () => {
    assert.deepEqual(browser.errors, []);
  }],
], async () => {
  if (browser) await browser.close();
  await agent.stop();
});
