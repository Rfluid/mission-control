// Test harness for the browser side: a sandboxed daemon and headless Chrome driven over the DevTools
// protocol. No npm dependencies (Node ≥ 22 has fetch and WebSocket). Modelled on redline's harness.

import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, existsSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname, isAbsolute } from "node:path";
import { fileURLToPath } from "node:url";

export const repo = join(dirname(fileURLToPath(import.meta.url)), "../..");
const win = process.platform === "win32";
const exe = process.env.MISSION_CONTROL_BIN || join(repo, `target/release/mission-control${win ? ".exe" : ""}`);
const CHROMES = ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser",
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  ...[process.env.PROGRAMFILES, process.env["PROGRAMFILES(X86)"], process.env.LOCALAPPDATA]
    .filter(Boolean).map((d) => join(d, "Google/Chrome/Application/chrome.exe"))];
export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
/** How long a DevTools socket may take to open, and a DevTools call to answer. */
const WS_OPEN_MS = 10000, SEND_MS = 30000;

/**
 * Agent side: the mission-control CLI against a throwaway state dir, user config dir, project dir and port.
 * Never touches ~/.local/state/mission-control or ~/.config/mission-control.
 */
export class Agent {
  constructor() {
    const root = mkdtempSync(join(tmpdir(), "mission-browser-"));
    this.root = root;
    this.project = join(root, "project");
    this.port = 20000 + Math.floor(Math.random() * 20000);
    for (const d of ["home", "config", "project"]) mkdirSync(join(root, d), { recursive: true });
    this.env = { ...process.env, MISSION_CONTROL_HOME: join(root, "home"), MISSION_CONTROL_CONFIG_DIR: join(root, "config"),
      MISSION_CONTROL_PORT: String(this.port), RUST_LOG: "mission_control=debug" };
    delete this.env.MISSION_CONTROL_ID;
  }

  /** Writes a file under the user config dir (`config/…`) or the project (`project/…`). */
  write(rel, text) {
    const p = join(this.root, rel);
    mkdirSync(dirname(p), { recursive: true });
    writeFileSync(p, text);
  }

  /** Starts `mission-control serve` in the foreground of this process (so we own its pid) and waits for /health. */
  async start() {
    // A random port can be taken (another test's daemon, anything else): pick another and retry.
    for (let attempt = 0; attempt < 5; attempt++) {
      this.daemon = spawn(exe, ["serve", "--port", String(this.port)], { env: this.env, stdio: ["ignore", "ignore", "pipe"] });
      this.daemonLog = "";
      this.daemon.stderr.on("data", (d) => { this.daemonLog += d; });
      for (let i = 0; i < 100 && this.daemon.exitCode === null; i++) {
        const ok = await fetch(`${this.url}/health`).then((r) => r.ok).catch(() => false);
        if (ok) return;
        await sleep(100);
      }
      if (!/Address already in use/i.test(this.daemonLog)) break;
      this.port = 20000 + Math.floor(Math.random() * 20000);
      this.env.MISSION_CONTROL_PORT = String(this.port);
    }
    throw new Error(`daemon didn't start:\n${this.daemonLog.slice(-2000)}`);
  }

  get url() { return `http://127.0.0.1:${this.port}`; }
  /** Runs the CLI; on failure the thrown error carries its stderr. */
  run(...args) {
    try {
      return execFileSync(exe, args, { env: this.env, encoding: "utf8", cwd: this.project, stdio: ["ignore", "pipe", "pipe"] });
    } catch (e) {
      e.message += `\n${e.stderr || ""}`;
      throw e;
    }
  }
  json(...args) { return JSON.parse(this.run(...args)); }

  /** `mission-control new` against the sandbox project; returns `{id, url}` and sets MISSION_CONTROL_ID for later calls. */
  newMission(title, template) {
    const r = this.json("new", title, ...(template ? ["-t", template] : []), "--project", this.project);
    this.env.MISSION_CONTROL_ID = r.mission.id;
    return { id: r.mission.id, url: r.url };
  }

  adminToken() { return readFileSync(join(this.env.MISSION_CONTROL_HOME, "admin.token"), "utf8").trim(); }

  async stop() {
    if (this.daemon && this.daemon.exitCode === null) {
      const exited = new Promise((r) => this.daemon.once("exit", r));
      this.daemon.kill();
      await Promise.race([exited, sleep(3000)]);
    }
    rmSync(this.root, { recursive: true, force: true });
  }
}

function findChrome() {
  if (process.env.CHROME) return process.env.CHROME;
  for (const c of CHROMES) {
    if (isAbsolute(c)) { if (existsSync(c)) return c; continue; }
    try { execFileSync(win ? "where" : "which", [c], { stdio: "ignore" }); return c; } catch {}
  }
  throw new Error("no Chrome found; set CHROME=/path/to/chrome");
}

/** Headless Chrome with one page, plus console-error and dialog tracking. */
export class Browser {
  /** Launches Chrome; a launch whose DevTools connection doesn't come up in time is retried once. */
  static async launch(opts = {}) {
    try {
      return await Browser.launchOnce(opts);
    } catch (e) {
      console.error(`# Chrome launch failed (${e.message.split("\n")[0]}), retrying once`);
      return Browser.launchOnce(opts);
    }
  }

  static async launchOnce({ dark = false, width = 1280, height = 900, mobile = false, media = [], scale = 0 } = {}) {
    const profile = mkdtempSync(join(tmpdir(), "mission-chrome-"));
    const proc = spawn(findChrome(), ["--headless=new", "--remote-debugging-port=0", `--user-data-dir=${profile}`,
      "--no-first-run", "--no-default-browser-check", "--disable-gpu", "--no-sandbox", "--hide-scrollbars", "about:blank"],
      { stdio: ["ignore", "ignore", "pipe"] });
    let stderr = "";
    proc.stderr.on("data", (d) => { stderr += d; });
    const portFile = join(profile, "DevToolsActivePort");
    const readPort = () => { try { return parseInt(readFileSync(portFile, "utf8"), 10) || 0; } catch { return 0; } };
    // A cold start (fresh profile, CI runners) can take well over 10s.
    let port = 0;
    for (let i = 0; i < 300 && !(port = readPort()) && proc.exitCode === null; i++) await sleep(100);
    if (!port) {
      proc.kill();
      throw new Error(`Chrome didn't open its DevTools port (exit ${proc.exitCode}):\n${stderr.slice(-2000)}`);
    }
    let target;
    for (let i = 0; i < 50 && !target; i++) {
      const list = await fetch(`http://127.0.0.1:${port}/json/list`).then((r) => r.json()).catch(() => []);
      target = list.find((t) => t.type === "page");
      if (!target) await sleep(100);
    }
    if (!target) {
      proc.kill();
      throw new Error("Chrome opened no page target");
    }
    const b = new Browser(proc, profile, new WebSocket(target.webSocketDebuggerUrl));
    // The socket can fail to open (the target went away): give up after a while instead of hanging.
    const opened = await Promise.race([
      new Promise((r) => b.ws.addEventListener("open", () => r(true), { once: true })),
      new Promise((r) => b.ws.addEventListener("close", () => r(false), { once: true })),
      sleep(WS_OPEN_MS).then(() => false),
    ]);
    if (!opened) {
      proc.kill();
      throw new Error("Chrome's DevTools socket didn't open");
    }
    await b.send("Runtime.enable");
    await b.send("Page.enable");
    await b.send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: scale || (mobile ? 2 : 1), mobile });
    const features = [...(dark ? [{ name: "prefers-color-scheme", value: "dark" }] : []), ...media];
    if (features.length) await b.send("Emulation.setEmulatedMedia", { features });
    return b;
  }

  constructor(proc, profile, ws) {
    Object.assign(this, { proc, profile, ws, id: 0, pending: new Map(), errors: [], dialogs: [] });
    // A closed socket answers nothing: fail what's waiting.
    ws.addEventListener("close", () => {
      for (const { reject } of this.pending.values()) reject(new Error("DevTools socket closed"));
      this.pending.clear();
    });
    ws.addEventListener("message", (m) => {
      const msg = JSON.parse(m.data);
      if (msg.id && this.pending.has(msg.id)) {
        const { resolve, reject } = this.pending.get(msg.id);
        this.pending.delete(msg.id);
        msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result);
      } else if (msg.method === "Runtime.exceptionThrown") {
        this.errors.push(msg.params.exceptionDetails.exception?.description || msg.params.exceptionDetails.text);
      } else if (msg.method === "Runtime.consoleAPICalled" && ["error", "warning"].includes(msg.params.type)) {
        this.errors.push(msg.params.args.map((a) => a.value ?? a.description).join(" "));
      } else if (msg.method === "Page.javascriptDialogOpening") {
        this.dialogs.push(msg.params.message);
        this.send("Page.handleJavaScriptDialog", { accept: true });
      }
    });
  }

  /** A DevTools call; one with no answer within SEND_MS fails (naming the method) instead of hanging. */
  send(method, params = {}) {
    const id = ++this.id;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error(`no answer to ${method} in ${SEND_MS / 1000}s`)); }, SEND_MS);
      this.pending.set(id, { resolve: (v) => { clearTimeout(timer); resolve(v); }, reject: (e) => { clearTimeout(timer); reject(e); } });
    });
  }

  async goto(url) {
    await this.send("Page.navigate", { url });
    await this.waitFor("document.readyState === 'complete'");
  }

  /** Evaluates `expr` (an expression, may be a promise) in the page; returns its JSON value. */
  async eval(expr) {
    const r = await this.send("Runtime.evaluate", { expression: expr, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error(`eval failed: ${r.exceptionDetails.exception?.description || r.exceptionDetails.text}\n  in: ${expr}`);
    return r.result.value;
  }

  /** Presses a key like a person: "Tab", "Enter", "Escape", "ArrowDown", or a printable character.
   *  `modifiers`: 1 Alt, 2 Ctrl, 4 Meta, 8 Shift. */
  async key(key, modifiers = 0) {
    const named = { Tab: 9, Enter: 13, Escape: 27, ArrowUp: 38, ArrowDown: 40, ArrowLeft: 37, ArrowRight: 39, Backspace: 8 };
    const code = named[key] ?? key.toUpperCase().charCodeAt(0);
    const text = key.length === 1 && !(modifiers & 6) ? key : key === "Enter" ? "\r" : undefined;
    const base = { key, code: key.length === 1 ? `Key${key.toUpperCase()}` : key, windowsVirtualKeyCode: code, modifiers };
    await this.send("Input.dispatchKeyEvent", { type: text ? "keyDown" : "rawKeyDown", ...base, ...(text && { text }) });
    await this.send("Input.dispatchKeyEvent", { type: "keyUp", ...base });
  }

  async waitFor(expr, timeout = 10000) {
    const end = Date.now() + timeout;
    let last;
    while (Date.now() < end) {
      last = await this.eval(expr).catch((e) => e.message);
      if (last === true) return;
      await sleep(50);
    }
    throw new Error(`timed out waiting for: ${expr} (last: ${JSON.stringify(last)})`);
  }

  /** PNG of the page (full height when `full`), written to `path`. */
  async screenshot(path, { full = false } = {}) {
    let clip;
    if (full) {
      const { contentSize } = await this.send("Page.getLayoutMetrics");
      clip = { x: 0, y: 0, width: contentSize.width, height: contentSize.height, scale: 1 };
    }
    const { data } = await this.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: full, ...(clip ? { clip } : {}) });
    writeFileSync(path, Buffer.from(data, "base64"));
  }

  async close() {
    const exited = this.proc.exitCode !== null || this.proc.signalCode !== null
      ? Promise.resolve()
      : new Promise((resolve) => this.proc.once("exit", resolve));
    // Chrome may drop the socket instead of answering Browser.close: don't wait on the reply.
    await Promise.race([this.send("Browser.close").catch(() => {}), sleep(2000)]);
    await Promise.race([exited, sleep(3000)]);
    this.proc.kill();
    await Promise.race([exited, sleep(2000)]);
    // Chrome's helper processes can still be writing the profile for a moment after it exits.
    try {
      rmSync(this.profile, { recursive: true, force: true, maxRetries: 20, retryDelay: 250 });
    } catch (e) {
      console.error(`# warning: could not remove ${this.profile}: ${e.code || e.message}`);
    }
  }
}

/** A step that takes longer than this fails (a hang shows which step, instead of stalling CI). */
const STEP_TIMEOUT_MS = 60000;

/** Runs named async steps in order; prints TAP-ish lines; exits 1 on the first failure. */
export async function steps(list, cleanup) {
  let failed = false;
  for (const [name, fn] of list) {
    let timer;
    const limit = new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error(`step took over ${STEP_TIMEOUT_MS / 1000}s`)), STEP_TIMEOUT_MS);
    });
    try { await Promise.race([fn(), limit]); console.log(`ok - ${name}`); }
    catch (e) { console.error(`FAIL - ${name}\n${e.stack || e}`); failed = true; break; }
    finally { clearTimeout(timer); }
  }
  await cleanup();
  if (failed) process.exit(1);
  console.log("all passed");
}
