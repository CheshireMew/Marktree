[CmdletBinding()]
param(
    [int]$TimeoutSeconds = 300
)

$ErrorActionPreference = "Stop"
$workspace = Split-Path -Parent $PSScriptRoot
if (-not $env:CARGO_HOME -and (Test-Path -LiteralPath "D:\Tools\Rust\cargo")) {
    $env:CARGO_HOME = "D:\Tools\Rust\cargo"
}
if (-not $env:RUSTUP_HOME -and (Test-Path -LiteralPath "D:\Tools\Rust\rustup")) {
    $env:RUSTUP_HOME = "D:\Tools\Rust\rustup"
}
if ($env:CARGO_HOME) {
    $cargoBin = Join-Path $env:CARGO_HOME "bin"
    if (Test-Path -LiteralPath $cargoBin) {
        $env:PATH = "$cargoBin;$env:PATH"
    }
}
$baseDirectory = if ($env:RUNNER_TEMP) {
    $env:RUNNER_TEMP
} elseif (Test-Path -LiteralPath "D:\Tools") {
    "D:\Tools"
} else {
    [System.IO.Path]::GetTempPath()
}
$runDirectory = Join-Path $baseDirectory ("marktree-native-smoke-" + [guid]::NewGuid().ToString("N"))
$smokeWorkspace = Join-Path $runDirectory "workspace"
New-Item -ItemType Directory -Path $smokeWorkspace -Force | Out-Null

$firstContent = "# Marktree native smoke`n`nSaved through the editor close lifecycle.`n"
$secondContent = "# Marktree native smoke`n`nReopened, rendered, and saved again.`n"
$document = Join-Path $smokeWorkspace "native-smoke.md"
$npm = (Get-Command npm.cmd -ErrorAction Stop).Source

function Stop-ProcessTree([int]$ProcessId) {
    $children = Get-CimInstance Win32_Process -Filter "ParentProcessId = $ProcessId" -ErrorAction SilentlyContinue
    foreach ($child in $children) {
        Stop-ProcessTree -ProcessId $child.ProcessId
    }
    Stop-Process -Id $ProcessId -Force -ErrorAction SilentlyContinue
}

function New-SmokeTauriConfig([string]$Phase) {
    $listener = [System.Net.Sockets.TcpListener]::new(
        [System.Net.IPAddress]::Loopback,
        0
    )
    $listener.Start()
    try {
        $port = ([System.Net.IPEndPoint]$listener.LocalEndpoint).Port
    } finally {
        $listener.Stop()
    }
    $configPath = Join-Path $runDirectory "$Phase.tauri.conf.json"
    $config = @{
        build = @{
            devUrl = "http://127.0.0.1:$port"
            beforeDevCommand = "npm run dev -- --host 127.0.0.1 --port $port"
        }
    } | ConvertTo-Json -Depth 3
    [System.IO.File]::WriteAllText($configPath, $config)
    return $configPath
}

function Invoke-SmokePhase([string]$Phase, [string]$ExpectedContent) {
    $env:VITE_MARKTREE_SMOKE_ROOT = $smokeWorkspace
    $env:VITE_MARKTREE_SMOKE_PHASE = $Phase
    $stdout = Join-Path $runDirectory "$Phase.stdout.log"
    $stderr = Join-Path $runDirectory "$Phase.stderr.log"
    $tauriConfig = New-SmokeTauriConfig -Phase $Phase
    $process = Start-Process `
        -FilePath $npm `
        -ArgumentList @("run", "desktop", "--", "--no-watch", "--config", $tauriConfig) `
        -WorkingDirectory $workspace `
        -WindowStyle Hidden `
        -RedirectStandardOutput $stdout `
        -RedirectStandardError $stderr `
        -PassThru

    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while (-not $process.HasExited -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 500
        $process.Refresh()
    }
    if (-not $process.HasExited) {
        Stop-ProcessTree -ProcessId $process.Id
        throw "The '$Phase' native smoke phase timed out. Logs: $stdout and $stderr"
    }
    $process.WaitForExit()
    if ($null -ne $process.ExitCode -and $process.ExitCode -ne 0) {
        throw "The '$Phase' native smoke phase exited with code $($process.ExitCode). Logs: $stdout and $stderr"
    }
    if (-not (Test-Path -LiteralPath $document -PathType Leaf)) {
        throw "The '$Phase' phase did not produce '$document'."
    }
    $actual = [System.IO.File]::ReadAllText($document)
    if ($actual -cne $ExpectedContent) {
        throw "The '$Phase' phase produced different document bytes. Evidence: $runDirectory"
    }
}

Invoke-SmokePhase -Phase "write" -ExpectedContent $firstContent
Invoke-SmokePhase -Phase "verify" -ExpectedContent $secondContent

Write-Host "Windows native smoke passed. Evidence retained at '$runDirectory'."
