//! Command line. Every agent verb exists here and as an MCP tool.

mod args;
mod commands;
mod more;

pub use args::{ConfigCmd, InstallArgs};

use clap::{Parser, Subcommand};

/// `0.2.0 (1a2b3c4d5e)`: the crate version and the commit it was built from (see build.rs).
pub const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (", env!("MISSION_GIT_SHA"), ")");

#[derive(Parser)]
#[command(name = "mission-control", version = VERSION, about = "Configurable mission control pages for coding-agent sessions")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(clap::Args)]
pub struct Target {
    /// Mission id (or set `MISSION_CONTROL_ID`).
    #[arg(short, long, env = "MISSION_CONTROL_ID")]
    pub mission: String,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the daemon in the foreground.
    Serve {
        #[arg(long, env = "MISSION_CONTROL_PORT", default_value_t = crate::paths::DEFAULT_PORT)]
        port: u16,
    },
    /// Daemon status (never starts one).
    Status,
    /// Stop the daemon (it restarts on the next command).
    Stop,
    /// Create a mission; prints its id and URL.
    New {
        title: String,
        #[arg(short, long)]
        template: Option<String>,
        /// Agent session that owns the mission (re-attach after a restart keeps the id).
        #[arg(long, env = "MISSION_CONTROL_SESSION")]
        session: Option<String>,
        /// Project root whose `.mission-control/` is the project layer (default: found from the working directory).
        #[arg(long)]
        project: Option<std::path::PathBuf>,
        /// Create even if this session already has a live mission with this title.
        #[arg(long)]
        force: bool,
        /// Turn on redline review for the page (needs redline installed).
        #[arg(long)]
        review: bool,
    },
    /// Comment anywhere on the page with redline: registers it as a redline artifact and loads the overlay.
    Review {
        #[command(flatten)]
        target: Target,
        /// Turn the overlay off again.
        #[arg(long)]
        off: bool,
        /// The redline CLI.
        #[arg(long, env = "MISSION_CONTROL_REDLINE", default_value = "redline")]
        redline: String,
    },
    /// Bind an existing mission to this session (after a restart): same id, records and history.
    Attach {
        id: String,
        #[arg(long, env = "MISSION_CONTROL_SESSION")]
        session: Option<String>,
    },
    /// List missions (`--all`: with archived ones and a summary of each).
    List {
        #[arg(long)]
        all: bool,
    },
    /// Archive a mission (`--restore` brings it back). It leaves the list; nothing is deleted.
    Archive {
        #[command(flatten)]
        target: Target,
        #[arg(long)]
        restore: bool,
    },
    /// A new mission from this one: same project and config, its records unless `--no-records`; fresh history.
    Fork {
        #[command(flatten)]
        target: Target,
        title: String,
        #[arg(long)]
        no_records: bool,
    },
    /// Print a mission's view (config, records, recent events) as JSON.
    View {
        #[command(flatten)]
        target: Target,
        /// Only records of these collections (repeatable).
        #[arg(short, long = "collection")]
        collections: Vec<String>,
        /// Newest events to include (default 100).
        #[arg(long)]
        events: Option<i64>,
        /// Leave the resolved config out.
        #[arg(long)]
        no_config: bool,
        /// Only this page: its sections, and records of the collections they read (plus `needs`).
        #[arg(long)]
        page: Option<String>,
    },
    /// Create or update a record: `mission-control upsert prs 17777 --state open title="Fix x" checks=green`.
    Upsert {
        #[command(flatten)]
        target: Target,
        collection: String,
        key: String,
        #[arg(short, long)]
        state: Option<String>,
        /// `field=value` pairs; a value that parses as JSON is stored as JSON.
        fields: Vec<String>,
    },
    /// Remove a record.
    Rm {
        #[command(flatten)]
        target: Target,
        collection: String,
        key: String,
    },
    /// Append to the activity log.
    Log {
        #[command(flatten)]
        target: Target,
        text: String,
        #[arg(short, long, default_value = "log")]
        kind: String,
    },
    /// Ask the user something (a record in `needs`, state `open`).
    Need {
        #[command(flatten)]
        target: Target,
        key: String,
        question: String,
        #[arg(long)]
        context: Option<String>,
        #[arg(short, long = "option")]
        options: Vec<String>,
        #[arg(long)]
        recommendation: Option<String>,
        #[arg(long)]
        why: Option<String>,
        /// What the question holds up: a record key (`U1`) or `collection/key`; repeatable.
        #[arg(long)]
        blocks: Vec<String>,
    },
    /// Apply a JSON array of ops from a file or `-` (stdin) in one transaction.
    Batch {
        #[command(flatten)]
        target: Target,
        file: String,
    },
    /// Configuration: layers, templates, provenance.
    #[command(subcommand)]
    Config(ConfigCmd),
    /// Block until the user answers or clicks something on the page; prints the page events.
    Wait {
        #[command(flatten)]
        target: Target,
        /// Whose cursor to continue (default `$MISSION_CONTROL_SESSION`, else `default`).
        #[arg(long, env = "MISSION_CONTROL_SESSION")]
        session: Option<String>,
        /// Start after this event seq instead of the session's cursor.
        #[arg(long)]
        since: Option<i64>,
        /// Seconds to block.
        #[arg(long, default_value_t = 120)]
        timeout: u64,
    },
    /// For Claude Code hooks: reads the hook's JSON on stdin and records subagents, PRs and spend. Never fails
    /// the hook (problems go to stderr, exit 0).
    Hook {
        /// Print the hooks block for Claude Code's settings.json instead.
        #[arg(long)]
        print_config: bool,
    },
    /// A short text digest to pick a mission back up: open needs, unread page events, `[brief]` collections.
    Brief {
        #[command(flatten)]
        target: Target,
        #[arg(long, env = "MISSION_CONTROL_SESSION")]
        session: Option<String>,
        /// Character budget (default `[brief] max_chars`).
        #[arg(long)]
        max_chars: Option<usize>,
    },
    /// MCP server on stdio, for agents (one per agent session).
    Mcp {
        /// Also push page events into a Claude Code session as channel messages.
        #[arg(long)]
        channel: bool,
    },
    /// Print the config's JSON Schema.
    Schema,
    /// Update to the latest release, or manage update notices.
    Update {
        #[command(subcommand)]
        action: Option<UpdateCmd>,
    },
    /// Link the skill and the CLI, and register the MCP server, for each coding agent.
    Install(InstallArgs),
    /// Undo `install`. Leaves missions and config alone.
    Uninstall(InstallArgs),
    /// Check this device's setup, then every live mission's health.
    Doctor(InstallArgs),
    /// Import an old mission-control `state.json` into a new mission (records, the activity log, the run).
    Import {
        file: std::path::PathBuf,
        /// Default: the file's `run.title`.
        #[arg(long)]
        title: Option<String>,
        #[arg(short, long, default_value = "multi-agent-build")]
        template: String,
        #[arg(long)]
        project: Option<std::path::PathBuf>,
        #[arg(long, env = "MISSION_CONTROL_SESSION")]
        session: Option<String>,
        /// Also write the suggested `[states]` for unknown states to the project config.
        #[arg(long)]
        write_config: bool,
    },
    /// Write a static, tokenless HTML snapshot of the page (one file, nothing to serve).
    Export {
        #[command(flatten)]
        target: Target,
        /// Where to write it (default stdout).
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum UpdateCmd {
    /// Check for a new release now and print what's known.
    Check,
    /// Stop notices for a release (default: the latest). A newer release still shows.
    Skip { version: Option<String> },
    /// Turn update checks and notices off.
    Off,
    /// Turn update checks and notices back on.
    On,
}

pub fn run() -> anyhow::Result<()> {
    commands::dispatch(Cli::parse().cmd)
}
