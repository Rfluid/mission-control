# Entry point on Windows (bin/mission-control is the Unix one; bin/mission-control.cmd and bin/mission-control run this there).
# Keeps target\bin\mission-control.exe current, then runs it. Agents' MCP configs and the CLI shims on PATH run
# that exe directly: Windows locks a running exe, so cargo could never rebuild one it launched itself.
# - With cargo: builds the release binary on first use, or when the sources changed.
# - Without cargo (or MISSION_CONTROL_PREBUILT=1): downloads this version's release binary once.
# Windows PowerShell 5.1 compatible (every Windows has it). Errors are checked explicitly: under a "Stop"
# preference, 5.1 turns any native program's stderr output into a terminating error.
trap { [Console]::Error.WriteLine("mission-control: $_"); exit 1 }
$repo = Split-Path -Parent $PSScriptRoot
$env:MISSION_CONTROL_REPO = $repo # the binary links this clone, not wherever it was built
$bin = Join-Path $repo 'target\bin'
$exe = Join-Path $bin 'mission-control.exe'

# Puts $src at $exe. The running copy can't be overwritten, but it can be renamed: move it aside and
# delete the leftovers once nothing runs them.
function Install-Exe([string]$src) {
  New-Item -ItemType Directory -Force $bin | Out-Null
  Get-ChildItem $bin -Filter 'mission-control.exe.old-*' -ErrorAction SilentlyContinue |
    Remove-Item -Force -ErrorAction SilentlyContinue
  if (Test-Path $exe) { Move-Item $exe "$exe.old-$([DateTime]::UtcNow.Ticks)" -Force -ErrorAction Stop }
  Copy-Item $src $exe -Force -ErrorAction Stop
  # The daemon handshake replaces a daemon started before its client's exe changed: date the copy now.
  (Get-Item $exe).LastWriteTimeUtc = [DateTime]::UtcNow
}

function Newer-Than([string]$path, [string[]]$sources) {
  $t = (Get-Item $path).LastWriteTimeUtc
  [bool](Get-ChildItem $sources -Recurse -File -ErrorAction SilentlyContinue |
    Where-Object { $_.LastWriteTimeUtc -gt $t } | Select-Object -First 1)
}

# Progress and errors go to stderr: stdout is the binary's (JSON, or MCP when an agent runs us).
function Build {
  & cargo build --release --quiet --manifest-path (Join-Path $repo 'Cargo.toml') | ForEach-Object { [Console]::Error.WriteLine($_) }
  if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
}

if ($env:MISSION_CONTROL_PREBUILT -ne '1' -and (Get-Command cargo -ErrorAction SilentlyContinue)) {
  $built = Join-Path $repo 'target\release\mission-control.exe'
  $sources = 'src', 'web', 'defaults', 'migrations', 'Cargo.toml', 'build.rs' | ForEach-Object { Join-Path $repo $_ }
  if (-not (Test-Path $built)) {
    Build
  } elseif (Newer-Than $built $sources) {
    if ($args.Count -gt 0 -and $args[0] -eq 'mcp' -and (Test-Path $exe)) {
      # Agents give an MCP server ~30s to start; a rebuild takes longer. Serve this session with the
      # current binary and rebuild in the background. The next run installs it; the daemon handshake
      # picks it up from there.
      $logdir = if ($env:MISSION_CONTROL_HOME) { $env:MISSION_CONTROL_HOME } else { Join-Path $env:LOCALAPPDATA 'mission-control' }
      New-Item -ItemType Directory -Force $logdir | Out-Null
      # A build still running from an earlier session holds the log open; it'll finish this one too.
      try {
        Start-Process cargo -WindowStyle Hidden -ErrorAction Stop `
          -RedirectStandardError (Join-Path $logdir 'build.log') -RedirectStandardOutput (Join-Path $logdir 'build.out.log') `
          -ArgumentList 'build', '--release', '--quiet', '--manifest-path', "`"$(Join-Path $repo 'Cargo.toml')`""
      } catch {}
    } else {
      Build
    }
  }
  if ((Test-Path $built) -and (-not (Test-Path $exe) -or (Get-Item $built).LastWriteTimeUtc -gt (Get-Item $exe).LastWriteTimeUtc)) {
    Install-Exe $built
  }
} else {
  $version = (Select-String -Path (Join-Path $repo 'Cargo.toml') -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches[0].Groups[1].Value
  $have = if (Test-Path $exe) { ((& $exe --version 2>$null) -split ' ')[1] } else { '' }
  if ($have -ne $version) {
    $arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
    $triple = switch ($arch) {
      'AMD64' { 'x86_64-pc-windows-msvc' }
      'ARM64' { 'aarch64-pc-windows-msvc' }
      default { [Console]::Error.WriteLine("mission-control: no release binary for Windows $arch; install Rust (https://rustup.rs)"); exit 1 }
    }
    $slug = if ($env:MISSION_CONTROL_RELEASE_REPO) { $env:MISSION_CONTROL_RELEASE_REPO } else { 'Rfluid/mission-control' }
    $name = "mission-control-$version-$triple.zip"
    $tmp = Join-Path ([IO.Path]::GetTempPath()) "mission-control-$([Guid]::NewGuid())"
    New-Item -ItemType Directory $tmp -ErrorAction Stop | Out-Null
    try {
      [Console]::Error.WriteLine("mission-control: downloading $name (no cargo found)")
      $gh = Get-Command gh -ErrorAction SilentlyContinue
      if ($gh) { & gh auth status *> $null }
      if ($gh -and $LASTEXITCODE -eq 0 -and -not $env:MISSION_CONTROL_RELEASE_BASE) {
        & gh release download "v$version" -R $slug -p $name -p "$name.sha256" -D $tmp |
          ForEach-Object { [Console]::Error.WriteLine($_) }
        if ($LASTEXITCODE -ne 0) { throw "no prebuilt binary for v$version (a tag can exist without a published release); install Rust (https://rustup.rs) to build from source" }
      } else {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        # MISSION_CONTROL_RELEASE_BASE: a mirror, or a local server in tests.
        $base = if ($env:MISSION_CONTROL_RELEASE_BASE) { $env:MISSION_CONTROL_RELEASE_BASE } else { "https://github.com/$slug/releases/download/v$version" }
        try {
          $ProgressPreference = 'SilentlyContinue'
          Invoke-WebRequest -UseBasicParsing "$base/$name" -OutFile (Join-Path $tmp $name) -ErrorAction Stop
          Invoke-WebRequest -UseBasicParsing "$base/$name.sha256" -OutFile (Join-Path $tmp "$name.sha256") -ErrorAction Stop
        } catch {
          [Console]::Error.WriteLine("mission-control: download failed: v$version may have no published release binaries, or the repo is private (``gh auth login``). Install Rust (https://rustup.rs) to build from source")
          exit 1
        }
      }
      $want = ((Get-Content (Join-Path $tmp "$name.sha256") -Raw).Trim() -split '\s+')[0]
      $got = (Get-FileHash (Join-Path $tmp $name) -Algorithm SHA256).Hash
      if ($got -ne $want) { [Console]::Error.WriteLine("mission-control: checksum mismatch for $name"); exit 1 }
      Expand-Archive (Join-Path $tmp $name) -DestinationPath $tmp -ErrorAction Stop
      Install-Exe (Join-Path $tmp "mission-control-$version-$triple\mission-control.exe")
    } finally {
      Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }
  }
}

# Windows PowerShell 5.1 (and 7.0-7.2) pass native arguments without escaping embedded quotes, and drop
# empty ones; 7.3+ ($PSNativeCommandArgumentPassing) gets them right on its own.
function Protect-Arg([string]$a) {
  if ($PSNativeCommandArgumentPassing -and $PSNativeCommandArgumentPassing -ne 'Legacy') { return $a }
  if ($a -eq '') { return '""' }
  if ($a -notmatch '\s') { return $a -replace '(\\*)"', '$1$1\"' }
  # With whitespace, quote it here and write inner quotes as "": 5.1 counts \" as a quote, so it would see the
  # whitespace as already quoted, add no quotes, and the argument would split. Inside quotes, "" is a literal quote.
  '"' + ($a -replace '(\\*)"', '$1$1""' -replace '(\\+)$', '$1$1') + '"'
}

& $exe @($args | ForEach-Object { Protect-Arg $_ })
exit $LASTEXITCODE
