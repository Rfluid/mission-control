# web/vendor

Third-party files compiled into the binary. Never edit them; replace them.

| File | What | From |
|---|---|---|
| `mermaid-11.12.3.min.js.gz` | Mermaid 11.12.3 (`dist/mermaid.min.js`, MIT, `LICENSE-mermaid.txt`), gzipped with `gzip -9 -n`. Served at `/assets/vendor/mermaid.js` with `Content-Encoding: gzip`; the page loads it only when markdown has a ` ```mermaid ` block | `npm pack mermaid@11.12.3`; sha256 of the unpacked file: `71dc54e481779b526de5e615c2eb00c99fa5acb75da9c5524d02e333d2fda89a` |

To update: `npm pack mermaid@<version>`, check its age (two weeks or more) and the license, `gzip -9 -n -c package/dist/mermaid.min.js > mermaid-<version>.min.js.gz`, update the sha256 here, the file name in `src/daemon/assets.rs` and `docs/DECISIONS.md`.

Mermaid's bundle includes its own dependencies (d3, DOMPurify, dagre, …) under their licenses (MIT, ISC, Apache-2.0 / MPL-2.0 for DOMPurify).
