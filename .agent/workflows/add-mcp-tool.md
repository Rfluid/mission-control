# Workflow: add an MCP tool

Invariant: every MCP tool has a CLI twin. The e2e MCP driver (`call <id> <tool> <args>` in `tests/e2e.sh`) sends one request and awaits its response: rmcp runs calls concurrently, so never pipe several calls in at once and expect order.

1. **Daemon route** (if new data or behaviour):
   - Add a handler in `src/daemon/handlers.rs`.
   - Register it in `router()` in `src/daemon/mod.rs`.
   - Auth: `admin` for agent-only writes; the mission token only for page events on that mission.
2. **Store** (if needed): a method on `Store`. Any multi-row write runs in one transaction (see CODE_STYLE).
3. **Client:** a typed method on `Api` in `src/api.rs`.
4. **Tool:** in `src/mcp/tools.rs`, add:
   - an args struct (`Deserialize + schemars::JsonSchema`; field doc comments become the schema descriptions)
   - a `#[tool(description = "...")]` method that goes through `blocking(|api| …)`.
   Descriptions are imperative and say when to use the tool. Config tools say which layer to write for which user phrasing.
5. **CLI twin:** a `Cmd` variant in `src/cli/mod.rs` plus a dispatch arm in `src/cli/commands.rs`.
6. **Tests:**
   - a unit test for any new store or config logic
   - an e2e step calling the tool over stdio
   - the CLI twin.
7. **Docs:** add the tool to `docs/PROTOCOL.md` (MCP tools + CLI tables) and to the `SKILL.md` tables.
