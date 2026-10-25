# Workflow: add a config key

1. **Name it** in the right table (`[mission]`, `[retention]`, a section prop, an action field, …). Prefer a prop on an existing table over a new top-level table. Decide which layers may set it; if it can run code or reach the network, it is user/project only (like `run` actions).
2. **Default:** put it in `defaults/config.toml` (or the template that owns it) with a one-line comment, unless "absent" is the right default.
3. **Merge behaviour:** check it works with the merge rules (tables merge; arrays of `id`'d tables merge by id; other arrays replace). If it's a list users will want to patch item by item, make the items tables with `id`.
4. **Validation:** add checks to `src/config/validate.rs`: errors for things that can't work or aren't allowed in a layer, warnings for dangling references. Unit-test both.
5. **Use it** (daemon, page or CLI), reading it from the resolved config, never from a single layer.
6. **Test:** a unit test that a user/project/mission layer can set it and `provenance` names that layer.
7. **Docs:** `docs/CONFIG.md` (the keys block and any table), and `SKILL.md` if agents should set it.
