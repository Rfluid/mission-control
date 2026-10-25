// Types for the page scripts (checked by `tsc --noEmit`, never shipped). Wire shapes mirror src/model.rs.

/** Config values are whatever TOML allowed; widgets read the keys they know. */
type Json = any;
type Config = { [key: string]: Json };

interface Mission {
  id: string;
  title: string;
  session: string | null;
  project_dir: string | null;
  created: number;
  archived: boolean;
}

interface Rec {
  collection: string;
  key: string;
  state: string | null;
  body: { [field: string]: Json };
  updated: number;
}

interface Ev {
  seq: number;
  mission: string;
  kind: string;
  text: string;
  data: Json;
  at: number;
}

interface Issue {
  severity: "error" | "warning";
  path: string;
  message: string;
}

interface View {
  mission: Mission;
  template: string;
  config: Config;
  issues: Issue[];
  records: Rec[];
  events: Ev[];
  event_count: number;
}

/** SSE `change` data. */
interface Change {
  records: Rec[];
  removed: { collection: string; key: string }[];
  events: Ev[];
  config: boolean;
}

/** A `[[sections]]` item: `id` and `widget`, plus the widget's own props. */
type Section = { id: string; widget: string; title?: string; [prop: string]: Json };

/** Count spec of a KPI tile: `{collection, state?, field?, value?}` or `{events: true}`. */
interface CountSpec {
  collection?: string;
  state?: string | string[];
  field?: string;
  value?: Json;
  events?: boolean;
}

interface Helpers {
  esc(s: unknown): string;
  time(ms: number | null | undefined): string;
  field(rec: Rec, f: string): Json;
  tone(ctx: Ctx, state: string | null | undefined): string;
  word(state: unknown): string;
  glyph(ctx: Ctx, state: string | null | undefined): string;
  pill(ctx: Ctx, state: string | null | undefined): string;
  fill(ctx: Ctx, tpl: string, value: Json, rec?: Rec): string;
  safeUrl(u: unknown): string | null;
  compare(x: Json, y: Json): number;
  /** Markdown → safe HTML. */
  md(src: unknown, opts?: { headings?: boolean }): string;
  table(cols: string[], rows: string[], empty: string): string;
  count(ctx: Ctx, spec: CountSpec): number;
  sum(ctx: Ctx, spec: { collection?: string; field: string | string[]; state?: string | string[] }): number;
  fmt(n: unknown, format?: string): string;
  metric(ctx: Ctx, spec: Json): { n: Json; of: number | null; text: string };
  ago(ms: number | null | undefined): string;
  find(ctx: Ctx, ref: unknown): Rec | null;
  title(rec: Rec): string;
  /** `label` is HTML (already escaped); default the key and title. */
  recordLink(rec: Rec, label?: string): string;
  toneRank(ctx: Ctx, state: string | null | undefined): number;
  [extra: string]: Json;
}

/** What every widget renders from. Widgets never fetch. */
interface Ctx {
  config: Config;
  records: { [collection: string]: Rec[] };
  events: Ev[];
  /** All events of the mission; `events` is only the newest page. */
  eventCount: number;
  /** Extra older events the user asked the feed for ("Older"). */
  more: number;
  /** Width of a full-width section's content box, in CSS px (charts draw at this size). */
  width: number;
  mission: Mission;
  /** The open page's id (`overview`, or a `[[pages]]` id). */
  page?: string;
  /** Reports a problem found in the data (a dependency cycle) in the config-issue banner. */
  issue?(path: string, message: string): void;
  h: Helpers;
  /** Buttons for action ids, optionally bound to a row. */
  buttons(ids: string[], row?: Rec): string;
}

type WidgetRender = (section: Section, ctx: Ctx) => string;
/** Text after a section's title ("1 open · 2 answered"); "" for none. */
type WidgetCount = (section: Section, ctx: Ctx) => string;
/** Controls for a section's header (HTML), e.g. a Graph / Table toggle. */
type WidgetTools = (section: Section, ctx: Ctx) => string;

interface MissionWidgetApi {
  h: Helpers;
  define(name: string, render: WidgetRender, opts?: { count?: WidgetCount; tools?: WidgetTools }): void;
  get(name: string): WidgetRender | undefined;
  count(name: string): WidgetCount | undefined;
  tools(name: string): WidgetTools | undefined;
}

interface Window {
  MISSION: { id: string; token: string; snapshot?: View };
  MissionWidget: MissionWidgetApi;
  /** Pages and tabs (web/pages.js). */
  MissionPages: {
    OVERVIEW: string;
    known(config: Config): string[];
    list(config: Config): { id: string; title: string; count?: CountSpec }[];
    sectionsOn(config: Config, page: string): Section[];
    current(config: Config): string;
    href(page: string): string;
    tabs(ctx: Ctx, page: string): string;
  };
  /** The shell: strip, live indicator (web/shell.js). */
  MissionShell: {
    render(ctx: Ctx, template: string): void;
    fresh(st: { state: string; last: number }): void;
    health(ctx: Ctx): { state: string | null; reason: string };
    needs(ctx: Ctx): { open: Rec[]; href: string };
  };
  /** The plan widget's phase summary, for the strip and the phase track (web/widgets-plan.js). */
  MissionPlan: {
    phase(section: Section, ctx: Ctx): { index: number; name: string; phases: string[] } | null;
    /** The plan model (phases, nodes, gates, tone helpers): see widgets-track.js. */
    model(section: Section, ctx: Ctx): any;
    mix(model: any, recs: Rec[]): string;
    counts(ctx: Ctx, model: any, recs: Rec[]): string;
    reqs(rec: Rec, field?: string): { text: string; done: boolean; ref: string | null }[];
  };
  /** Zoom, fit, fullscreen and pan for diagrams (web/viewport.js). */
  MissionViewport: {
    controls(id: string): string;
    style(id: string): string;
    save(el: Element): void;
    restore(el: Element): void;
    zoomTo(id: string, z: number, cx?: number, cy?: number): void;
    fit(id: string): void;
  };
  /** The record drawer (web/drawer.js). */
  MissionDrawer: { attach(get: () => Ctx | null): void; refresh(): void; show(ref: string, section?: string, from?: HTMLElement | null): void; close(): void };
  /** Draws ```mermaid figures (web/mermaid.js). */
  MissionMermaid: { run(root: ParentNode): Promise<void> };
  /** Command palette (web/palette.js). */
  MissionPalette: { attach(get: () => Ctx | null): void; open(): void; close(restore?: boolean): void; go(page: string, then?: () => void): void };
  /** Keyboard shortcuts (web/keys.js). */
  MissionKeys: { attach(get: () => Ctx | null): void; letters(config: Config): Map<string, string> };
  /** Title, favicon, notifications, live-region text (web/attention.js). */
  MissionAttention: { update(ctx: Ctx): void; observe(ctx: Ctx, before: Rec[], after: Rec[]): void };
  /** Live updates (web/live.js). */
  MissionLive: {
    connect(config: Config, on: { change(c: Change): void; reload(): void; status(): void }): void;
    status(): { state: string; last: number };
    touch(): void;
  };
  /** State glyphs (web/glyphs.js). */
  MissionGlyph: { glyph(tone: string): string; diamond(tone: string, size?: number): string; ORDER: string[] };
  /** Safe markdown → HTML (web/markdown.js). */
  MissionMarkdown: { render(src: unknown, opts?: { headings?: boolean }): string };
  /** Click handling for buttons and answers (web/actions.js). */
  MissionActions: { attach(getCtx: () => Ctx | null): void };
}
