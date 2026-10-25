//! Argument groups shared by several commands: `install`/`uninstall`/`doctor`, and the `config` subcommands.

use super::Target;
use clap::Subcommand;

#[allow(clippy::struct_excessive_bools)] // independent command-line switches
#[derive(clap::Args)]
pub struct InstallArgs {
    /// all (default: detected) or a comma list: claude,codex,opencode,cursor,agy,gemini
    #[arg(long)]
    agents: Option<String>,
    /// Extra Claude config dir (a second profile); repeatable.
    #[arg(long)]
    claude_config_dir: Vec<std::path::PathBuf>,
    /// Extra skills dir to link into; repeatable.
    #[arg(long)]
    skill_dir: Vec<std::path::PathBuf>,
    #[arg(long, default_value = "~/.local/bin")]
    bin_dir: String,
    #[arg(long)]
    force: bool,
    /// Only link the skill and CLI; don't touch agent MCP configs.
    #[arg(long)]
    no_mcp: bool,
    /// Print what would change; write nothing.
    #[arg(long)]
    dry_run: bool,
    /// Claude Code: push page events into the session (`mission-control mcp --channel`).
    #[arg(long)]
    channel: bool,
}

impl InstallArgs {
    pub fn into_opts(self) -> crate::install::Opts {
        crate::install::Opts {
            agents: self.agents,
            claude_config_dirs: self.claude_config_dir,
            skill_dirs: self.skill_dir,
            bin_dir: crate::install::expand(&self.bin_dir),
            force: self.force,
            no_mcp: self.no_mcp,
            dry_run: self.dry_run,
            channel: self.channel,
        }
    }
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Resolved config without a mission (preview defaults): builtin + template + user + project.
    Resolve {
        #[arg(short, long)]
        template: Option<String>,
        #[arg(long)]
        project: Option<std::path::PathBuf>,
    },
    /// A mission's resolved config, or the part at a key path (`sections[prs].columns`).
    Get {
        #[command(flatten)]
        target: Target,
        path: Option<String>,
    },
    /// Every resolved key and the layer that set it.
    Explain {
        #[command(flatten)]
        target: Target,
        /// Only keys starting with this path.
        prefix: Option<String>,
    },
    /// Patch a layer with a TOML or JSON table (validated; versioned or backed up, so undoable).
    Patch {
        #[command(flatten)]
        target: Target,
        patch: String,
        #[arg(long)]
        note: Option<String>,
        /// mission (this page), user (every mission), project (this repo's missions).
        #[arg(long, default_value = "mission", value_parser = ["mission", "user", "project"])]
        layer: String,
        /// Validate and preview (returns a preview URL); store nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Save this mission's customizations as a template (`extends` its template).
    TemplateSave {
        #[command(flatten)]
        target: Target,
        name: String,
        #[arg(long, default_value = "user", value_parser = ["user", "project"])]
        layer: String,
        #[arg(long)]
        description: Option<String>,
        /// Overwrite an existing template file of that name.
        #[arg(long)]
        replace: bool,
    },
    /// Define a named widget: a built-in with preset props (`--base chart --props '{"kind":"line"}'`) or HTML.
    Widget {
        #[command(flatten)]
        target: Target,
        name: String,
        #[arg(long, conflicts_with = "html", required_unless_present = "html")]
        base: Option<String>,
        /// Props as a JSON object (with `--base`).
        #[arg(long, requires = "base")]
        props: Option<String>,
        /// HTML shown in a sandboxed iframe.
        #[arg(long)]
        html: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long, default_value = "mission", value_parser = ["mission", "user", "project"])]
        layer: String,
    },
    /// Undo the last change to a layer.
    Undo {
        #[command(flatten)]
        target: Target,
        #[arg(long, default_value = "mission", value_parser = ["mission", "user", "project"])]
        layer: String,
    },
    /// A layer's changes, newest first.
    History {
        #[command(flatten)]
        target: Target,
        #[arg(long, default_value = "mission", value_parser = ["mission", "user", "project"])]
        layer: String,
    },
    /// Templates available here, and the layers that define each.
    Templates {
        #[arg(long)]
        project: Option<std::path::PathBuf>,
    },
}
