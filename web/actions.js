// @ts-check
// Buttons and answers. `open` and `copy` run here; everything else is a page event the daemon interprets from
// the config and hands to the agent (`wait`, channel push). Sent with the mission token in `X-Mission-Token`.
(function () {
  "use strict";
  const { id, token } = window.MISSION;
  const TOAST_MS = 2600;

  /** @param {string} text @param {boolean} [bad] */
  function toast(text, bad) {
    const t = /** @type {HTMLElement} */ (document.getElementById("toast"));
    t.textContent = text;
    t.className = `toast show${bad ? " bad" : ""}`;
    clearTimeout(Number(t.dataset.timer));
    t.dataset.timer = String(setTimeout(() => { t.className = "toast"; }, TOAST_MS));
  }

  /** @param {object} body */
  async function send(body) {
    const r = await fetch(`/api/missions/${encodeURIComponent(id)}/events`, {
      method: "POST",
      headers: { "content-type": "application/json", "x-mission-token": token },
      body: JSON.stringify(body),
    });
    if (!r.ok) throw new Error(await r.text());
    return /** @type {{ok: boolean, seq: number | null, exit: number | null}} */ (await r.json());
  }

  /** Asks for text in the page's dialog; resolves to the text, or null if cancelled.
   *  @param {string} title @param {string} hint @param {boolean} required */
  function ask(title, hint, required) {
    const dlg = /** @type {HTMLDialogElement} */ (document.getElementById("prompt"));
    const area = /** @type {HTMLTextAreaElement} */ (dlg.querySelector("textarea"));
    /** @type {HTMLElement} */ (dlg.querySelector("h3")).textContent = title;
    area.placeholder = hint;
    area.required = required;
    area.value = "";
    return new Promise(resolve => {
      dlg.onclose = () => resolve(dlg.returnValue === "send" ? area.value : null);
      dlg.showModal();
      area.focus();
    });
  }

  /** `{row.x}`, `{value}`, `{vars.x}` filled as plain text (for `copy`; links use `h.fill`).
   *  @param {Ctx} ctx @param {string} tpl @param {Rec | undefined} rec */
  function text(ctx, tpl, rec) {
    return String(tpl).replace(/\{([a-z_.]+)\}/gi, (_, k) => {
      if (k.startsWith("vars.")) return String((ctx.config.vars || {})[k.slice(5)] ?? "");
      const v = k === "value" ? rec && rec.key : k.startsWith("row.") && rec ? ctx.h.field(rec, k.slice(4)) : "";
      return v == null ? "" : typeof v === "object" ? JSON.stringify(v) : String(v);
    });
  }

  /** @param {Ctx} ctx @param {HTMLElement} b */
  async function action(ctx, b) {
    const name = b.dataset.action || "", act = (ctx.config.actions || {})[name];
    if (!act) return;
    const rec = b.dataset.row != null
      ? (ctx.records[b.dataset.collection || ""] || []).find(r => r.key === b.dataset.row) : undefined;
    const label = act.label || name;
    // `run` executes a command on this machine: always ask.
    if ((act.confirm || act.kind === "run") && !window.confirm(`${label}${rec ? ` (${rec.key})` : ""}?`)) return;
    if (act.kind === "open") {
      const url = ctx.h.safeUrl(ctx.h.fill(ctx, act.url || "", rec && rec.key, rec));
      if (url) window.open(url, "_blank", "noopener");
      else toast(`${label}: no http(s) link`, true);
      return;
    }
    if (act.kind === "copy") {
      await navigator.clipboard.writeText(text(ctx, act.text || "", rec));
      toast("Copied");
      return;
    }
    /** @type {string | null | undefined} */
    let typed;
    if (act.kind === "customize") typed = await ask(label, "What should change on this page?", true);
    else if (act.kind === "ask_agent" && (act.prompt || !act.message)) typed = await ask(label, act.placeholder || "Anything to add? (optional)", false);
    if (typed === null) return;
    const res = await send({ kind: "action", action: name, collection: rec && rec.collection, key: rec && rec.key, text: typed || undefined });
    if (act.kind === "run") toast(`${label}: ${res.exit == null ? "failed" : `exit ${res.exit}`}`, res.exit !== 0);
    else toast(act.kind === "set" ? `${label}: done` : "Sent to the agent");
  }

  /** An answer button: its own value, or the need's text box when it has none.
   *  @param {HTMLElement} b */
  async function answer(b) {
    const need = b.dataset.need || "";
    let value = b.dataset.answer;
    if (value == null) {
      const box = /** @type {HTMLInputElement | null} */ (b.closest(".need")?.querySelector("[data-draft]") || null);
      value = box ? box.value.trim() : "";
      if (!value) return void toast("Type an answer first", true);
    }
    await send({ kind: "answer", need, collection: b.dataset.collection || undefined, answer: value });
    toast(`Answered ${need}`);
  }

  window.MissionActions = {
    /** @param {() => Ctx | null} getCtx */
    attach(getCtx) {
      document.addEventListener("click", async e => {
        const b = /** @type {HTMLElement | null} */ (e.target instanceof Element ? e.target.closest("button[data-action], button[data-need]") : null);
        const ctx = getCtx();
        if (!b || !ctx || b.hasAttribute("disabled")) return;
        b.setAttribute("disabled", "");
        try {
          if (b.dataset.need != null) await answer(b);
          else await action(ctx, b);
        } catch (err) {
          toast(`Not sent: ${err instanceof Error ? err.message : err}`, true);
        } finally {
          b.removeAttribute("disabled");
        }
      });
      document.addEventListener("keydown", e => {
        const t = e.target instanceof HTMLInputElement ? e.target : null;
        if (e.key === "Enter" && t && t.dataset.draft) {
          /** @type {HTMLElement | null} */ (t.closest(".need")?.querySelector("button[data-send]") || null)?.click();
        }
      });
    },
  };
})();
