# Prompt: debug a problem

```
Something is wrong in mission-control: <symptom, exact error, steps>.
Work in an isolated sandbox (MISSION_CONTROL_HOME=$(mktemp -d), MISSION_CONTROL_CONFIG_DIR=$(mktemp -d), MISSION_CONTROL_PORT=7499); never touch ~/.local/state/mission-control or ~/.config/mission-control.
Reproduce it first with the smallest command sequence or test. For config problems, start with `mission-control config explain -m <id> [path]` (which layer set what) and `mission-control config resolve --template <t>`. Find the root cause, fix it, and add a regression test (unit or tests/e2e.sh). Run `just check`. Report the cause, the fix, and the test.
```
