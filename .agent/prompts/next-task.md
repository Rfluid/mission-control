# Prompt: continue the roadmap

Paste into a fresh agent session at the repo root:

```
You are developing mission-control in this repo. Read .agent/README.md and follow it: read .agent/context.md and .agent/rules.md and the newest entry in .agent/log/, then run `just check`.
Take the next unchecked task in the current phase of docs/ROADMAP.md and complete it using .agent/workflows/implement-task.md.
Stop after that one task: tick it in the roadmap, write a .agent/log/ entry, and report what changed, how you verified it, and what's next. Ask me before anything that edits my agent configs or my ~/.config/mission-control, commits, or changes the protocol, the config format or the security model.
```

Variant, for a specific task: replace the second paragraph with `Implement roadmap task <ID> (docs/ROADMAP.md).`
