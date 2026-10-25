// Mission page in headless Chrome: config layers drive what renders.
// Run: `just browser` (builds the release binary first), or `node tests/browser/page.test.mjs`.

import assert from "node:assert/strict";
import { Agent, Browser, steps } from "./harness.mjs";

const agent = new Agent();
let browser, mission;

// User layer: a button on every PR row. Project layer: the repo for PR links, a custom state color, and a
// fast refresh so config changes show up within the test's timeouts.
agent.write("config/config.toml", `
[actions.rerun-ci]
label = "Re-run CI"
kind = "ask_agent"
message = "Re-run CI on {row.key}"

[[sections]]
id = "prs"
row_actions = ["rerun-ci"]
`);
agent.write("project/.mission-control/config.toml", `
[mission]
refresh = "poll:1"

[vars]
repo = "acme/widgets"

[states]
at_risk = "warn"
`);

const sectionIds = () => browser.eval(`[...document.querySelectorAll("#root section")].map(s => s.id)`);
/** Section ids the daemon puts on a page (`?page=`), in config order. */
const resolvedIds = async (page = "overview") => (await fetch(`${agent.url}/api/missions/${mission.id}?page=${page}`).then((r) => r.json()))
  .config.sections.map((s) => s.id);
const tabs = () => browser.eval(`[...document.querySelectorAll("#nav a.tab")].map(a => [a.dataset.page, a.getAttribute("aria-current")])`);

await steps([
  ["daemon starts in a sandbox", async () => {
    await agent.start();
    mission = agent.newMission("Browser test", "multi-agent-build");
    assert.match(mission.url, new RegExp(`^${agent.url}/m/msn_`));
  }],

  ["records are written", () => {
    agent.run("upsert", "packages", "R0", "-s", "merged", "name=Contracts", "wave=W0", "pr=17777");
    agent.run("upsert", "packages", "U2", "-s", "in_progress", "name=Review UI", "wave=W1",
      "note=<script>window.pwned=1</script><img src=x onerror=\"window.pwned=2\">");
    agent.run("upsert", "prs", "17777", "-s", "open", "title=feat: contracts", "pkg=R0", "checks=green");
    agent.run("upsert", "run", "run", "-s", "at_risk", "reason=cap drift");
    agent.run("need", "N-1", "Ship now or after review?", "-o", "now", "-o", "later", "--recommendation", "later");
    agent.run("log", "Run started");
  }],

  ["page loads without errors", async () => {
    browser = await Browser.launch();
    await browser.goto(mission.url);
    await browser.waitFor(`document.querySelectorAll("#root section").length > 0`);
    assert.deepEqual(browser.errors, []);
  }],

  ["the open page's sections render in config order, and the tabs list the pages", async () => {
    const want = await resolvedIds();
    assert.deepEqual(await sectionIds(), want);
    assert.ok(!want.includes("status"), "multi-agent-build removes minimal's status section");
    const t = await tabs();
    assert.deepEqual(t[0], ["overview", "page"], "Overview first and current");
    assert.equal(await browser.eval(`document.querySelector("nav.tabs").getAttribute("aria-label")`), "Pages");
  }],

  ["pills take their tone from [states], including a project-defined state", async () => {
    assert.equal(await browser.eval(`document.querySelector("#strip .cell.hl .pill").className`), "pill c-warn");
    await browser.goto(`${mission.url}/plan`);
    await browser.waitFor(`!!document.querySelector('#plan [data-plan-view="table"]')`);
    await browser.eval(`document.querySelector('#plan [data-plan-view="table"]').click()`);
    await browser.waitFor(`!!document.querySelector("#plan table .pill")`);
    const merged = await browser.eval(`[...document.querySelectorAll("#plan .pill")].find(p => p.textContent === "merged")?.className`);
    assert.equal(merged, "pill c-done");
    const prog = await browser.eval(`[...document.querySelectorAll("#plan .pill")].find(p => p.textContent === "in progress")?.className`);
    assert.equal(prog, "pill c-prog");
  }],

  ["the PR link is built from the project's vars.repo", async () => {
    await browser.goto(`${mission.url}/prs`);
    await browser.waitFor(`!!document.querySelector("#prs a")`);
    const href = await browser.eval(`document.querySelector("#prs a")?.href`);
    assert.equal(href, "https://github.com/acme/widgets/pull/17777");
  }],

  ["the user layer's row action shows on PR rows", async () => {
    const label = await browser.eval(`document.querySelector('#prs button[data-action="rerun-ci"]')?.textContent`);
    assert.equal(label, "Re-run CI");
    assert.equal(await browser.eval(`document.querySelector('#prs button[data-action="rerun-ci"]').dataset.row`), "17777");
  }],

  ["record text is escaped, never run", async () => {
    await browser.goto(`${mission.url}/plan`);
    await browser.waitFor(`!!document.querySelector("#plan table .pill")`, 5000);
    assert.equal(await browser.eval(`typeof window.pwned`), "undefined");
    const note = await browser.eval(`document.querySelector("#plan").textContent`);
    assert.ok(note.includes("<script>window.pwned=1</script>"), "the note shows as text");
    assert.equal(await browser.eval(`document.querySelectorAll("#plan script, #plan img").length`), 0);
  }],

  ["every pill shows its glyph and its word", async () => {
    await browser.goto(`${mission.url}/plan`);
    await browser.waitFor(`document.querySelectorAll("#root .pill").length > 0`);
    const pills = await browser.eval(`[...document.querySelectorAll("body .pill")].map(p => [!!p.querySelector("svg.g[aria-hidden=true]"), p.textContent.trim()])`);
    assert.ok(pills.length >= 4, `pills: ${pills.length}`);
    for (const [glyph, word] of pills) assert.ok(glyph && word, `pill "${word}" glyph=${glyph}`);
  }],

  ["the fonts come from the binary, and nothing loads from another origin", async () => {
    await browser.waitFor(`document.fonts.status === "loaded"`);
    const loaded = await browser.eval(`[...document.fonts].filter(f => f.status === "loaded").map(f => f.family.replace(/"/g, ""))`);
    assert.ok(loaded.includes("Inter") && loaded.includes("JetBrains Mono"), `loaded: ${loaded}`);
    assert.match(await browser.eval(`getComputedStyle(document.body).fontFamily`), /^Inter/);
    const foreign = await browser.eval(`performance.getEntriesByType("resource").map(e => e.name).filter(n => !n.startsWith(location.origin))`);
    assert.deepEqual(foreign, []);
    const font = await fetch(`${agent.url}/assets/fonts/inter-v20-latin.woff2`);
    assert.equal(font.headers.get("content-type"), "font/woff2");
    assert.match(await (await fetch(`${agent.url}/assets/fonts/OFL-inter.txt`)).text(), /SIL Open Font License/);
  }],

  ["a mission-control config patch removing a section applies on the next refresh", async () => {
    await browser.goto(`${mission.url}/decisions`);
    await browser.waitFor(`!!document.getElementById("risks")`);
    agent.run("config", "patch", '{"sections":[{"id":"risks","remove":true}]}', "--note", "no risks");
    await browser.waitFor(`!document.getElementById("risks")`);
    assert.deepEqual(await sectionIds(), await resolvedIds("decisions"));
    await browser.goto(mission.url);
    await browser.waitFor(`[...document.querySelectorAll("#recent .row")].some(r => r.textContent.includes("no risks"))`);
  }],

  ["a mission-layer patch adds a page and moves a section onto it, live", async () => {
    agent.run("config", "patch", '{"pages":[{"id":"notes","title":"Notes","count":{"collection":"decisions"}}],"sections":[{"id":"decisions","page":"notes"}]}');
    await browser.waitFor(`!!document.querySelector('#nav a[data-page="notes"]')`);
    assert.ok(!(await sectionIds()).includes("decisions"), "moved off Overview");
    assert.equal(await browser.eval(`document.querySelector('#nav a[data-page="notes"] .cnt').textContent`), "0");
    await browser.eval(`document.querySelector('#nav a[data-page="notes"]').click()`);
    await browser.waitFor(`location.pathname.endsWith("/notes")`);
    assert.deepEqual(await sectionIds(), ["decisions"]);
    assert.deepEqual((await tabs()).find(([p]) => p === "notes"), ["notes", "page"]);
    await browser.goto(`${mission.url}/notes`);
    await browser.waitFor(`document.querySelectorAll("#root section").length === 1`);
    assert.deepEqual(await sectionIds(), await resolvedIds("notes"));
    await browser.eval(`history.back()`);
    await browser.waitFor(`!location.pathname.endsWith("/notes")`);
    await browser.goto(mission.url);
    await browser.waitFor(`document.querySelectorAll("#root section").length > 1`);
    const explain = agent.run("config", "explain", "pages");
    assert.match(explain, /pages\[notes\]\.title\s+←\s+mission v\d/);
  }],

  ["an unknown widget shows the config-issue banner", async () => {
    agent.run("config", "patch", '{"sections":[{"id":"odd","widget":"nope","after":"overview"}]}');
    await browser.waitFor(`document.querySelector("#issues .banner")?.textContent.includes("unknown widget")`);
    assert.equal(await browser.eval(`document.querySelector("#issues .banner").classList.contains("warn")`), true);
    assert.ok((await browser.eval(`document.getElementById("odd").textContent`)).includes("not available yet"));
  }],

  ["a run action in the mission layer is refused", () => {
    assert.throws(() => agent.run("config", "patch", '{"actions":{"x":{"kind":"run","command":["ls"]}}}'),
      /only in user\/project/);
  }],

  ["the theme button switches to dark tokens", async () => {
    assert.equal(await browser.eval(`getComputedStyle(document.body).backgroundColor`), "rgb(247, 247, 245)");
    await browser.eval(`document.getElementById("theme").click()`);
    assert.equal(await browser.eval(`document.documentElement.dataset.theme`), "dark");
    assert.equal(await browser.eval(`getComputedStyle(document.body).backgroundColor`), "rgb(14, 14, 15)");
  }],

  ["the OS dark preference applies without the button", async () => {
    const dark = await Browser.launch({ dark: true });
    try {
      await dark.goto(mission.url);
      await dark.waitFor(`document.querySelectorAll("#root section").length > 0`);
      assert.equal(await dark.eval(`getComputedStyle(document.body).backgroundColor`), "rgb(14, 14, 15)");
    } finally { await dark.close(); }
  }],

  ["the home lists missions with what needs the user; archived ones are under Finished", async () => {
    const other = agent.json("new", "Archived one", "--project", agent.project).mission.id;
    agent.run("archive", "-m", other);
    await browser.goto(agent.url + "/");
    await browser.waitFor(`document.querySelectorAll(".mcard").length >= 1`);
    assert.match(await browser.eval(`document.querySelector(".mgrid").innerText`), /Browser test[\s\S]*1 needs you/);
    assert.ok(!(await browser.eval(`document.querySelector(".mgrid").innerText`)).includes("Archived one"));
    await browser.eval(`document.querySelector('[data-filter="finished"]').click()`);
    assert.match(await browser.eval(`document.querySelector(".mgrid").innerText`), /Archived one/);
    await browser.eval(`document.querySelector('[data-filter="active"]').click()`);
  }],

  ["export writes a static snapshot that renders from a file, with no token", async () => {
    const file = `${agent.root}/snapshot.html`;
    agent.run("export", "-o", file);
    const html = (await import("node:fs")).readFileSync(file, "utf8");
    const token = await (await fetch(mission.url)).text().then((t) => /token: "([0-9a-f]{32})"/.exec(t)[1]);
    assert.ok(!html.includes(token), "the mission token isn't in the snapshot");
    assert.ok(!html.includes('src="/assets/'), "assets are inlined");
    const snap = await Browser.launch();
    try {
      await snap.goto(`file://${file}`);
      await snap.waitFor(`document.querySelectorAll("#root section").length > 3`);
      assert.match(await snap.eval(`document.getElementById("fresh").textContent`), /^snapshot of/);
      assert.equal(await snap.eval(`[...document.querySelectorAll("#root button[data-action]")].every(b => b.disabled)`), true);
      assert.ok((await snap.eval(`document.getElementById("inflight").textContent`)).includes("<script>window.pwned=1</script>"),
        "a record's </script> stays text and doesn't end the embedded data");
      assert.equal(await snap.eval(`typeof window.pwned`), "undefined");
      await snap.eval(`document.querySelector('#nav a[data-page="plan"]').click()`);
      await snap.waitFor(`!!document.getElementById("plan")`);
      assert.deepEqual(snap.errors, []);
    } finally { await snap.close(); }
  }],

  ["no console errors along the way", () => {
    assert.deepEqual(browser.errors, []);
  }],
], async () => {
  if (browser) await browser.close();
  await agent.stop();
});
