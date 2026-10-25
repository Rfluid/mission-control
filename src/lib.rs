//! mission: configurable mission control pages for coding-agent sessions.
//!
//! The binary (`src/main.rs`) is a thin wrapper over [`cli::run`]. Modules:
//! - [`config`]: layered config (builtin → template → user → project → mission), merge, provenance, validation.
//! - [`store`]: `SQLite` persistence (Diesel): missions, records, events, mission-control config versions.
//! - [`daemon`]: local HTTP server (`mission-control serve`): pages and the agent API.
//! - [`api`]: blocking agent-side client; [`lifecycle`] starts, replaces and stops the daemon.
//! - [`mcp`]: MCP server on stdio, one per agent session.
//! - [`install`]: links the skill and CLI and registers the MCP server for each coding agent.

pub mod api;
pub mod cli;
pub mod config;
pub mod daemon;
pub mod hooks;
pub mod ids;
pub mod import;
pub mod install;
pub mod layerfile;
pub mod lifecycle;
pub mod mcp;
pub mod model;
pub mod paths;
pub mod store;
pub mod update;
