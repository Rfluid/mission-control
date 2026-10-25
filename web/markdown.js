// @ts-check
// Markdown for record text (ported from redline's web/markdown.js): escape first, then add a fixed set of
// tags: paragraphs, line breaks, **bold**, *italic*, `code`, fenced code blocks, - / 1. lists, # headings (bold
// lines, or h3/h4 with `headings`) and [links](https://…). No HTML from the input survives; links take http(s)
// and mailto only. A ```mermaid block becomes a figure holding its (escaped) source; web/mermaid.js draws it.
(function () {
  "use strict";
  /** @param {unknown} s */
  const esc = s => String(s).replace(/[&<>"']/g, c => /** @type {Record<string, string>} */ ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
  const SAFE_URL = /^(https?:\/\/|mailto:)/i;

  /** `text` is already escaped. Code spans are cut out first so nothing inside them is formatted.
   *  @param {string} text */
  function inline(text) {
    return text.split(/(`[^`\n]+`)/).map((part, i) => {
      if (i % 2) return `<code>${part.slice(1, -1)}</code>`;
      return part
        .replace(/\[([^\]\n]+)\]\(([^)\s]+)\)/g, (m, label, url) =>
          SAFE_URL.test(url) ? `<a href="${url}" target="_blank" rel="noopener noreferrer">${label}</a>` : m)
        .replace(/\*\*([^*\n]+)\*\*/g, "<strong>$1</strong>")
        .replace(/(^|[^\w*])\*([^*\n]+)\*(?!\w)/g, "$1<em>$2</em>")
        .replace(/(^|[^\w])_([^_\n]+)_(?!\w)/g, "$1<em>$2</em>");
    }).join("");
  }

  /** @param {string} chunk @param {boolean} headings @returns {string} */
  function block(chunk, headings) {
    const lines = chunk.split("\n");
    if (lines.every(l => /^\s*[-*] /.test(l))) return `<ul>${lines.map(l => `<li>${inline(l.replace(/^\s*[-*] /, ""))}</li>`).join("")}</ul>`;
    if (lines.every(l => /^\s*\d+[.)] /.test(l))) return `<ol>${lines.map(l => `<li>${inline(l.replace(/^\s*\d+[.)] /, ""))}</li>`).join("")}</ol>`;
    const h = /^(#{1,6}) (.*)$/.exec(lines[0]);
    // A heading directly followed by text: the heading, then the rest as its own block.
    if (h && lines.length > 1) return block(lines[0], headings) + block(lines.slice(1).join("\n"), headings);
    if (h && lines.length === 1) return headings ? `<h${h[1].length <= 3 ? 3 : 4}>${inline(h[2])}</h${h[1].length <= 3 ? 3 : 4}>` : `<p><strong>${inline(h[2])}</strong></p>`;
    return `<p>${lines.map(inline).join("<br>")}</p>`;
  }

  /** Markdown source → safe HTML string. `headings`: `#` lines become h3/h4 (long-form docs).
   *  @param {unknown} src @param {{headings?: boolean}} [opts] */
  function render(src, opts = {}) {
    // split() with two groups: text, language, code, text, language, code, …
    const parts = esc(String(src ?? "").replace(/\r\n?/g, "\n")).split(/^```([^\n]*)\n([\s\S]*?)^```[ \t]*$/m);
    const out = [];
    for (let i = 0; i < parts.length; i += 3) {
      out.push(parts[i].split(/\n[ \t]*\n/).map(c => c.trim()).filter(Boolean).map(c => block(c, !!opts.headings)).join(""));
      if (i + 2 < parts.length) {
        const code = parts[i + 2].replace(/\n$/, "");
        out.push(parts[i + 1].trim() === "mermaid"
          ? `<figure class="mmd"><pre class="mmd-src"><code>${code}</code></pre></figure>`
          : `<pre><code>${code}</code></pre>`);
      }
    }
    return out.join("");
  }

  window.MissionMarkdown = { render };
})();
