# mission task runner. `just` lists recipes; `just check` is what CI runs.

default:
    @just --list

# Format Rust sources
fmt:
    cargo fmt

# Lint: rustfmt check, clippy (warnings are errors), JS syntax and types, design tokens (contrast + sync)
lint: types
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    for f in web/*.js tests/browser/*.mjs tests/web/*.mjs docs/screenshots/*.mjs; do node --check "$f"; done
    python3 .design/check.py

# Type-check the page (web/*.js: JSDoc + @ts-check). Installs the pinned typescript on first run
types:
    test -d node_modules/typescript || npm ci --no-audit --no-fund
    npx --no-install tsc -p .

# Dependency audit (deny.toml): advisories, licenses, bans, sources. Needs `cargo install cargo-deny`
deny:
    cargo deny check

# Unit tests (Rust, and the page's pure modules in Node)
test:
    cargo test
    node --test tests/web/*.test.mjs

# End-to-end test (temp state and config dirs, random port)
e2e:
    cargo build --release
    bash tests/e2e.sh
    bash tests/customize.sh

# Browser tests in headless Chrome (CHROME=/path to override): the mission page rendered from config
browser:
    cargo build --release
    node tests/browser/page.test.mjs
    node tests/browser/live.test.mjs
    node tests/browser/widgets.test.mjs
    node tests/browser/shell.test.mjs
    node tests/browser/plan.test.mjs
    node tests/browser/drawer.test.mjs
    node tests/browser/architecture.test.mjs
    node tests/browser/keys.test.mjs
    node tests/browser/attention.test.mjs
    node tests/browser/home.test.mjs

# Regenerate the README screenshots in docs/screenshots/ (sandboxed daemon, headless Chrome)
screenshots:
    cargo build --release
    node docs/screenshots/shoot.mjs

# Everything CI checks
check: lint test e2e browser

# Bump versions + CHANGELOG for the next release (args: --bump minor, --pre beta, --dry-run, ...)
prepare-release *args:
    scripts/prepare-release.sh {{args}}

# Run a sandboxed daemon in the foreground (never touches ~/.local/state/mission-control or ~/.config/mission-control)
dev port="7499":
    MISSION_CONTROL_HOME="$(mktemp -d)" MISSION_CONTROL_CONFIG_DIR="$(mktemp -d)" cargo run -- serve --port {{port}}
