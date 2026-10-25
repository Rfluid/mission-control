//! Where mission keeps state and finds config on this device.

use crate::model::DaemonState;
use anyhow::Result;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_PORT: u16 = 7466;

/// `$MISSION_CONTROL_HOME`, else `~/.local/state/mission-control` (platform equivalent where there is one). Created if missing.
pub fn state_dir() -> Result<PathBuf> {
    let dir = state_dir_path();
    if dir.as_os_str().is_empty() {
        anyhow::bail!("no home directory");
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// [`state_dir`] without creating it (empty if there's no home directory).
pub fn state_dir_path() -> PathBuf {
    match std::env::var_os("MISSION_CONTROL_HOME").filter(|d| !d.is_empty()) {
        Some(d) => PathBuf::from(d),
        None => dirs::state_dir()
            .or_else(|| if cfg!(windows) { dirs::data_local_dir() } else { None })
            .or_else(|| dirs::home_dir().map(|h| h.join(".local/state")))
            .map(|d| d.join("mission-control"))
            .unwrap_or_default(),
    }
}

/// The user config layer: `$MISSION_CONTROL_CONFIG_DIR`, else `$XDG_CONFIG_HOME/mission`, else `~/.config/mission-control`.
/// Holds `config.toml`, `templates/`, `widgets/`.
pub fn user_config_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("MISSION_CONTROL_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .map(|d| d.join("mission-control"))
}

/// The user layer's `config.toml` (also where `[update]` lives).
pub fn user_config_file() -> PathBuf {
    user_config_dir().unwrap_or_default().join("config.toml")
}

/// The nearest directory at or above `start` that has a `.mission-control/` folder, else the git root, else `start`.
pub fn project_root(start: &Path) -> PathBuf {
    let has = |name: &str| start.ancestors().find(|d| d.join(name).exists()).map(Path::to_path_buf);
    has(".mission-control").or_else(|| has(".git")).unwrap_or_else(|| start.to_path_buf())
}

/// The clone this binary belongs to: the skill dir agents link to, and where `bin/mission-control` lives.
/// `$MISSION_CONTROL_REPO` (set by `bin/mission-control`), else the checkout around the binary (`<repo>/target/<dir>/mission`),
/// else where it was built. A downloaded release binary has a build path from CI, so the first two matter.
pub fn repo_dir() -> PathBuf {
    if let Some(r) = std::env::var_os("MISSION_CONTROL_REPO").filter(|r| !r.is_empty()) {
        return PathBuf::from(r);
    }
    let exe = std::env::current_exe().and_then(|e| e.canonicalize()).ok().map(plain);
    exe.and_then(|e| checkout_of(&e)).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

/// What agents' MCP configs and the CLI on PATH run.
/// - Unix: `<repo>/bin/mission-control`, which rebuilds (or downloads) the binary when needed.
/// - Windows: `<repo>\target\bin\mission-control.exe`, the copy `bin\mission-control.ps1` keeps current. A script can't sit in
///   an MCP stdio path there.
pub fn entry_point(repo: &Path) -> PathBuf {
    if cfg!(windows) {
        repo.join("target").join("bin").join("mission-control.exe")
    } else {
        repo.join("bin").join("mission-control")
    }
}

/// A clone script (`bin/mission-control`, `scripts/checkout-release.sh`): the file, and the argv that runs it.
/// On Windows that's its `.ps1` twin under Windows `PowerShell`, which every Windows has.
pub fn script(unix_script: &Path) -> (PathBuf, Vec<OsString>) {
    if cfg!(windows) {
        let ps1 = unix_script.with_extension("ps1");
        let mut argv: Vec<OsString> =
            ["powershell.exe", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"]
                .map(OsString::from)
                .into();
        argv.push(ps1.clone().into_os_string());
        (ps1, argv)
    } else {
        (unix_script.to_path_buf(), vec![unix_script.as_os_str().to_owned()])
    }
}

/// Drops the `\\?\` prefix Windows' `canonicalize` adds (`\\?\C:\x` → `C:\x`), so paths we hand to agents,
/// scripts and junctions are ordinary ones. UNC paths and every path elsewhere pass through.
pub fn plain(p: PathBuf) -> PathBuf {
    match p.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => p,
    }
}

/// `<repo>` for `<repo>/target/<dir>/mission`, if `<repo>` is a mission checkout.
fn checkout_of(exe: &Path) -> Option<PathBuf> {
    let target = exe.ancestors().nth(2)?;
    let repo = target.parent()?;
    (target.file_name()? == "target" && repo.join("SKILL.md").is_file() && repo.join("bin/mission-control").is_file())
        .then(|| repo.to_path_buf())
}

pub fn read_daemon_state() -> Option<DaemonState> {
    serde_json::from_slice(&fs::read(state_dir().ok()?.join("daemon.json")).ok()?).ok()
}

pub fn write_daemon_state(s: &DaemonState) -> Result<()> {
    write_private(&state_dir()?.join("daemon.json"), &serde_json::to_vec_pretty(s)?)
}

pub fn remove_daemon_state() -> Result<()> {
    match fs::remove_file(state_dir()?.join("daemon.json")) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

/// Agent-side token for writes. Stable across daemon restarts; readable only by this user.
pub fn admin_token() -> Result<String> {
    let p = state_dir()?.join("admin.token");
    if let Ok(t) = fs::read_to_string(&p)
        && !t.trim().is_empty()
    {
        return Ok(t.trim().to_string());
    }
    let t = crate::ids::new_token();
    write_private(&p, t.as_bytes())?;
    Ok(t)
}

/// Writes `p` atomically, readable only by this user.
fn write_private(p: &Path, data: &[u8]) -> Result<()> {
    use std::io::Write;
    let tmp = p.with_extension("tmp");
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let mut f = opts.open(&tmp)?;
    f.write_all(data)?;
    drop(f);
    fs::rename(tmp, p)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkout_is_found_from_a_binary_inside_it() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        fs::create_dir_all(repo.join("bin")).unwrap();
        fs::create_dir_all(repo.join("target/prebuilt")).unwrap();
        fs::write(repo.join("SKILL.md"), "x").unwrap();
        fs::write(repo.join("bin/mission-control"), "x").unwrap();
        assert_eq!(checkout_of(&repo.join("target/prebuilt/mission-control")).as_deref(), Some(repo));
        assert_eq!(checkout_of(&repo.join("elsewhere/a/mission")), None);
    }

    #[test]
    fn plain_drops_only_the_verbatim_drive_prefix() {
        assert_eq!(plain(PathBuf::from(r"\\?\C:\r\mission")), PathBuf::from(r"C:\r\mission"));
        assert_eq!(plain(PathBuf::from(r"\\?\UNC\host\share")), PathBuf::from(r"\\?\UNC\host\share"));
        assert_eq!(plain(PathBuf::from("/home/u/mission")), PathBuf::from("/home/u/mission"));
    }
}
