param(
    [int]$TargetBpm = 137,
    [string]$CodexBinary = 'codex',
    [string]$Model = 'gpt-5.6-terra',
    [int]$DebugPort = 9222,
    [switch]$KeepChange
)

$ErrorActionPreference = 'Stop'

function Step([string]$Message) {
    Write-Host "[ghost-fl-codex-test] $Message"
}

if ($TargetBpm -lt 20 -or $TargetBpm -gt 300) {
    throw 'TargetBpm must be between 20 and 300 for this bounded smoke test.'
}

Step 'This test lets Codex use only fl_get_tempo and fl_set_tempo against the live FL Studio project.'
Step 'Use a scratch project. Keep Gopher available and do not run the Gopher agent concurrently with Ghost.'
if (-not $KeepChange) {
    Step 'The original project tempo must be an integer; Ghost will restore it automatically after the Codex turn.'
}

Step 'Running adapter unit tests ...'
& cargo test -p ghost-fl-studio
if ($LASTEXITCODE -ne 0) {
    throw "ghost-fl-studio tests failed with exit code $LASTEXITCODE"
}

$args = @(
    'run', '--quiet', '-p', 'ghost-fl-agent-smoke', '--',
    '--debug-port', "$DebugPort",
    '--target-bpm', "$TargetBpm",
    '--codex-binary', $CodexBinary,
    '--model', $Model
)
if ($KeepChange) {
    $args += '--keep-change'
}

Step "Starting live Codex -> Ghost -> FL Studio test at target tempo $TargetBpm BPM ..."
$previousErrorActionPreference = $ErrorActionPreference
try {
    # Rust/Codex diagnostics may use stderr even on a successful run.
    $ErrorActionPreference = 'Continue'
    & cargo @args
    $exitCode = $LASTEXITCODE
}
finally {
    $ErrorActionPreference = $previousErrorActionPreference
}

if ($exitCode -ne 0) {
    throw "Codex FL Studio smoke test failed with exit code $exitCode"
}

Step 'GREEN: live Codex FL Studio smoke completed.'
