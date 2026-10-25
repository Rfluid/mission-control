// The demo missions (the design mockups' scenario): "Billing export build" mid-run, plus a few others for the
// missions home. Browser tests, `docs/screenshots/shoot.mjs` and manual checks all seed from here, so they
// look at the same data. Writes go through the CLI (`batch`), like an agent's.

import { join } from "node:path";

const MIN = 60000;

/** Upsert op. */
const up = (collection, key, state, patch) => ({ op: "upsert", collection, key, ...(state && { state }), patch });
const log = (text, kind) => ({ op: "log", text, ...(kind && { kind }) });

/** Applies `ops` to the agent's current mission in one batch. */
function apply(agent, ops, name = "ops") {
  const file = join(agent.root, `${name}.json`);
  agent.write(`${name}.json`, JSON.stringify(ops));
  agent.run("batch", file);
}

/** The project config the demo mission adds on top of the template: the repo for links. */
export const DEMO_PROJECT = `
[vars]
repo = "acme/billing"
`;

const SEQUENCE = `sequenceDiagram
  participant Screen
  participant API
  participant Job
  participant S3
  Screen->>API: POST /exports
  API->>Job: enqueue(job)
  Job->>S3: put csv
  API-->>Screen: 200 link`;

const DOC = `### Request path
The export screen posts a filter to \`POST /exports\`. The API checks the session, writes an \`export_jobs\` row and enqueues it; the screen polls the job until it has a download link.

### The nightly job
Runs at 02:00 UTC, reads the ledger in pages of 10k rows, writes one CSV per tenant to S3 and signs a 24-hour link. Retries three times with backoff; a fourth failure opens a need.

\`\`\`mermaid
${SEQUENCE}
\`\`\`

### Open questions
Whether archived invoices belong in the export is with you (N-3).`;

/** Tasks in the order they were created, then the state changes that brought them where they are (so the
 *  feed and the drawer history read like a real run). */
const TASKS = [
  ["C0", { name: "Contracts and schema", phase: "P1", agent: "c0-contracts", pr: 412 }],
  ["C1", { name: "Test fixtures", phase: "P1", agent: "lead" }],
  ["A1", { name: "Export API", phase: "P2", agent: "a1-api", pr: 418, depends_on: ["C0"] }],
  ["S1", { name: "Storage adapter", phase: "P2", agent: "s1-store", depends_on: ["C0"], branch: "feat/s3-adapter" }],
  ["U1", { name: "Export screen", phase: "P2", agent: "u1-ui", pr: 421, depends_on: ["C1"] }],
  ["J1", { name: "Nightly job", phase: "P3", agent: "j1-job", depends_on: ["A1"] }],
  ["D1", { name: "Docs", phase: "P3", depends_on: ["U1"] }],
  ["R1", { name: "Release", phase: "P4", depends_on: ["J1", "D1", "S1"] }],
];
const MOVES = [
  ["C0", "in_progress"], ["C1", "in_progress"], ["C1", "done"], ["C0", "merged", "PR 412 merged"],
  ["A1", "in_progress"], ["S1", "in_progress", "s1-store picked it up."], ["U1", "in_progress"],
  ["A1", "merged", "PR 418 merged"], ["J1", "in_progress"], ["U1", "in_review", "PR 421, one thread open"],
  ["S1", "blocked", "Integration test needs AWS_* secrets."],
];

/** Seeds "Billing export build" (template multi-agent-build) and returns `{id, url}`. */
export function seedDemo(agent) {
  agent.write("project/.mission-control/config.toml", DEMO_PROJECT);
  const m = agent.newMission("Billing export build", "multi-agent-build");
  const ops = [
    up("run", "run", "on_track", { reason: "Build phase merging; storage adapter blocked on credentials." }),
    up("phases", "P1", null, { title: "Contracts" }),
    up("phases", "P2", null, { title: "Build" }),
    up("phases", "P3", null, { title: "Run" }),
    up("phases", "P4", null, { title: "Ship" }),
    ...TASKS.map(([k, body]) => up("packages", k, "todo", body)),
    log("Run started from the approved design"),
  ];
  for (const [k, state, text] of MOVES) {
    ops.push(up("packages", k, state, {}));
    if (text) ops.push(log(`${k}: ${text}`));
  }
  ops.push(
    up("packages", "S1", null, { reason: "S3 credentials missing in CI", note: "Adapter is done locally; only the CI job fails. Once the secret lands the agent re-runs and opens the PR." }),
    up("packages", "J1", null, { note: "Retry policy in progress" }),
    up("packages", "U1", null, { note: "One review thread open" }),
    up("gates", "G0", "passed", { title: "Contracts → Build", after: "P1", requirements: [{ text: "Contracts merged", done: true }, { text: "Schema reviewed", done: true }] }),
    up("gates", "G1", "pending", { title: "Build → Run", after: "P2", requirements: [{ text: "API merged", done: true, ref: "A1" }, { text: "Export screen approved", done: false, ref: "U1" }, { text: "Storage adapter merged", done: false, ref: "S1" }] }),
    up("gates", "G2", "todo", { title: "Run → Ship", after: "P3", requirements: [{ text: "Nightly job green three runs", done: false, ref: "J1" }, { text: "Docs reviewed", done: false, ref: "D1" }] }),
    up("needs", "N-1", "answered", { question: "CSV or Parquet first?", options: ["CSV", "Parquet"], recommendation: "CSV", answer: "CSV" }),
    up("needs", "N-2", "answered", { question: "How long should export links last?", options: ["24 hours", "7 days"], recommendation: "24 hours", answer: "24 hours" }),
    up("needs", "N-3", "open", {
      question: "Should exports include archived invoices?",
      context: "Finance asked for them; support says they confuse customers. Changes the export schema, so U1 waits on it.",
      options: ["Include them, flagged", "Leave them out", "Separate export"],
      recommendation: "Include them, flagged",
      why: "Finance needs the full ledger, and a flag column keeps support's concern visible.",
      blocks: "U1",
    }),
    up("prs", "412", "merged", { title: "feat: export contracts", pkg: "C0", review: "APPROVED", checks: "green" }),
    up("prs", "418", "merged", { title: "feat: export API", pkg: "A1", review: "APPROVED", checks: "green" }),
    up("prs", "421", "open", { title: "feat: export screen", pkg: "U1", review: "CHANGES_REQUESTED", checks: "green" }),
    up("agents", "c0-contracts", "finished", { pkg: "C0", last_report: "Merged 41ac0d2" }),
    up("agents", "a1-api", "finished", { pkg: "A1", last_report: "Merged 9d69bf4" }),
    up("agents", "s1-store", "finished", { pkg: "S1", last_report: "Blocked: needs AWS_* secrets in CI" }),
    up("agents", "u1-ui", "running", { pkg: "U1", last_report: "Addressing the date-range review comment" }),
    up("agents", "j1-job", "running", { pkg: "J1", last_report: "Backoff with jitter; 41 tests pass" }),
    up("tests", "A1-unit", "pass", { pkg: "A1", layer: "unit", count: 64 }),
    up("tests", "U1-e2e", "pass", { pkg: "U1", layer: "e2e", count: 12 }),
    up("tests", "S1-int", "fail", { pkg: "S1", layer: "integration", count: 3 }),
    up("decisions", "D-2", null, { title: "Pages of 10k rows", why: "Keeps job memory under 200 MB.", by: "lead" }),
    up("decisions", "D-3", null, { title: "Signed links expire in 24h", why: "Security review asked for it.", by: "you" }),
    up("decisions", "D-4", null, { title: "CSV first, Parquet later", why: "Finance opens them in Sheets.", by: "you" }),
    up("decisions", "D-5", null, { title: "Queue in Postgres, not Redis", why: "One fewer service to run.", by: "lead" }),
    up("risks", "K-1", "open", { title: "Large accounts may exceed the 15 min job window", owner: "lead", fallback: "Chunk by month" }),
    up("spend", "lead-claude-opus-5-5", null, { model: "claude-opus-5-5", usd: 11.6, input_tokens: 1900000, output_tokens: 160000 }),
    up("spend", "agents-claude-sonnet-5-5", null, { model: "claude-sonnet-5-5", usd: 2.6, input_tokens: 950000, output_tokens: 90000 }),
    ...[8, 8, 7, 7, 6, 6, 5, 5].map((open, i) => up("burn", `t${i}`, null, { at: Date.now() - (7 - i) * 220 * MIN, open, done: 8 - open })),
    up("components", "screen", null, { title: "Export screen", group: "Browser", task: "U1" }),
    up("components", "admin", null, { title: "Admin page", group: "Browser", note: "existing" }),
    up("components", "api", null, { title: "Export API", group: "API", task: "A1" }),
    up("components", "auth", null, { title: "Auth", group: "API", note: "existing" }),
    up("components", "job", null, { title: "Nightly job", group: "Workers", task: "J1" }),
    up("components", "queue", null, { title: "Export queue", group: "Workers", note: "new · in A1" }),
    up("components", "pg", null, { title: "Postgres", group: "Storage", note: "ledger" }),
    up("components", "s3", null, { title: "S3 bucket", group: "Storage", task: "S1" }),
    up("links", "l1", null, { from: "screen", to: "api", label: "HTTP" }),
    up("links", "l2", null, { from: "admin", to: "auth", label: "session" }),
    up("links", "l3", null, { from: "api", to: "queue", label: "enqueue" }),
    up("links", "l4", null, { from: "queue", to: "job", label: "pulls" }),
    up("links", "l5", null, { from: "job", to: "pg", label: "reads" }),
    up("links", "l6", null, { from: "job", to: "s3", label: "writes CSV" }),
    up("docs", "architecture", null, { title: "How exports work", body: DOC }),
    log("G0 Contracts → Build passed", "gate"),
    log("#421 opened by u1-ui", "pr"),
  );
  apply(agent, ops, "demo");
  return m;
}

/** Seeds the other missions on the home page: one blocked, one at risk, three finished (archived). */
export function seedHome(agent) {
  const others = [];
  const babysit = agent.newMission("PR babysit: auth-v2", "pr-babysit");
  apply(agent, [
    up("run", "run", "blocked", { reason: "CI red on #512 after main moved" }),
    up("prs", "510", "merged", { title: "feat: token refresh", checks: "green" }),
    up("prs", "511", "merged", { title: "feat: session store", checks: "green" }),
    up("prs", "512", "open", { title: "feat: revoke on logout", checks: "red" }),
    up("needs", "N-1", "open", { question: "Rebase onto main or merge main in?", options: ["Rebase", "Merge"], recommendation: "Rebase", why: "History stays linear; the branch is short.", blocks: "512" }),
  ], "babysit");
  others.push(babysit);
  const hunt = agent.newMission("Flaky test hunt", "research");
  apply(agent, [
    up("run", "run", "at_risk", { reason: "Slower than planned" }),
    ...["timeouts", "clock skew", "port reuse"].map((q, i) => up("questions", `Q${i + 1}`, "answered", { question: `Is it ${q}?`, answer: "Ruled out" })),
    up("questions", "Q4", "in_progress", { question: "Is it the shared tmp dir?" }),
    ...["Q5", "Q6", "Q7"].map((k) => up("questions", k, "open", { question: "Not started" })),
  ], "hunt");
  others.push(hunt);
  for (const [title, template, state] of [["Docs site migration", "minimal", "done"], ["Rate limiter", "multi-agent-build", "done"], ["Search relevance spike", "research", "closed"]]) {
    const m = agent.newMission(title, template);
    apply(agent, [up("run", "run", state, { reason: "Finished" }), log("Finished")], "done");
    agent.run("archive", "-m", m.id);
    others.push(m);
  }
  return others;
}
