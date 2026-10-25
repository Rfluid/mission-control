// Regenerates the screenshots in this folder: the demo missions (tests/browser/demo.mjs, the design mockups'
// scenario) in headless Chrome against a sandboxed daemon (never your real state or config). Desktop shots are
// 1440 px wide at device scale 2; phone shots 390 × 844. Each waits for the fonts and the live stream to settle.
// Run: `just screenshots` (builds the release binary first). Needs Chrome (or CHROME=/path).

import { join } from "node:path";
import { Agent, Browser, repo, sleep } from "../../tests/browser/harness.mjs";
import { seedDemo, seedHome } from "../../tests/browser/demo.mjs";

const OUT = join(repo, "docs/screenshots");
const DESKTOP = { width: 1440, scale: 2 }, PHONE = { width: 390, height: 844, mobile: true };
const SETTLE_MS = 700; // after fonts and data: let the stream's first change and any re-render land

/** name, page path, window, what must be drawn, and what to do before the shot. */
const SHOTS = [
  ["overview-light", "", { ...DESKTOP, height: 1000 }, "#needs .need"],
  ["overview-dark", "", { ...DESKTOP, height: 1000, dark: true }, "#needs .need"],
  ["plan-light", "/plan", { ...DESKTOP, height: 860 }, "#plan .pnode"],
  ["plan-dark", "/plan", { ...DESKTOP, height: 860, dark: true }, "#plan .pnode"],
  ["architecture-light", "/architecture", { ...DESKTOP, height: 1180 }, "#howitworks figure.mmd svg"],
  ["drawer-light", "/plan?record=packages/S1", { ...DESKTOP, height: 900 }, "#drawer .hist li"],
  ["palette-light", "", { ...DESKTOP, height: 900 }, "#needs .need", async (b) => {
    await b.key("k", 2);
    for (const c of "export") await b.key(c);
    await b.waitFor(`document.querySelectorAll("#palette .pal-i").length > 3`);
  }],
  ["phone-overview", "", PHONE, "#needs .need"],
  ["phone-plan", "/plan", PHONE, "#plan .pnode"],
];

const agent = new Agent();
try {
  await agent.start();
  const demo = seedDemo(agent);
  seedHome(agent);
  const shots = [...SHOTS, ["home-light", "@home", { ...DESKTOP, height: 900 }, ".mcard"]];
  for (const [name, path, opts, ready, before] of shots) {
    const b = await Browser.launch(opts);
    try {
      await b.goto(path === "@home" ? `${agent.url}/` : demo.url + path);
      await b.waitFor(`document.fonts.status === "loaded" && !!document.querySelector(${JSON.stringify(ready)})`, 30000);
      await sleep(SETTLE_MS);
      if (before) await before(b);
      await sleep(200);
      await b.screenshot(join(OUT, `${name}.png`));
      console.log(`wrote docs/screenshots/${name}.png`);
      if (b.errors.length) throw new Error(`${name}: console errors: ${b.errors.join(" | ")}`);
    } finally {
      await b.close().catch(() => {});
    }
  }
} finally {
  await agent.stop();
}
