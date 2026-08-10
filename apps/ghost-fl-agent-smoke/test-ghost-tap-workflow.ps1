[CmdletBinding()]
param(
    [int]$TapInstance = 0,
    [double]$CaptureSeconds = 4.0,
    [string]$Track = "1",
    [int]$SlotStart = 1,
    [int]$SlotEnd = 4,
    [string[]]$Plugin = @("Pro-Q 4", "Pro-C 3"),
    [string]$Intent = "Build a minimal, musical processor chain for this sample from the measured evidence. Improve clarity, balance, dynamics, and usefulness in a mix without erasing its character.",
    [string]$Model = "gpt-5.6-terra",
    [string]$CodexBinary = "codex",
    [int]$DebugPort = 9222,
    [switch]$IHavePositionedPlayheadAndConfirmedEmptySlots
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not $IHavePositionedPlayheadAndConfirmedEmptySlots) {
    throw "Safety acknowledgement required. Position the playhead immediately before the sample, stop transport, keep Ghost Tap after the writable range (recommended slot 10), confirm target slots are empty, then pass -IHavePositionedPlayheadAndConfirmedEmptySlots."
}

$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Push-Location $repoRoot
try {
    Write-Host "[ghost-tap-workflow] Checking Ghost Tap + analysis + FL adapter + parallel App Server runtime ..."
    & cargo check -p ghost-clap-plugin -p ghost-core -p ghost-codex -p ghost-fl-studio -p ghost-fl-agent-smoke
    if ($LASTEXITCODE -ne 0) {
        throw "cargo check failed with exit code $LASTEXITCODE"
    }

    $arguments = @(
        "run", "--quiet", "-p", "ghost-fl-agent-smoke", "--bin", "ghost-fl-workflow", "--",
        "--debug-port", [string]$DebugPort,
        "--tap-instance", [string]$TapInstance,
        "--capture-seconds", $CaptureSeconds.ToString([Globalization.CultureInfo]::InvariantCulture),
        "--track", $Track,
        "--slot-start", [string]$SlotStart,
        "--slot-end", [string]$SlotEnd,
        "--intent", $Intent,
        "--model", $Model,
        "--codex-binary", $CodexBinary,
        "--i-have-positioned-playhead-and-confirmed-empty-slots"
    )
    foreach ($name in $Plugin) {
        $arguments += "--plugin"
        $arguments += $name
    }

    Write-Host "[ghost-tap-workflow] Running capture -> analysis -> Codex processor thread -> FL chain workflow ..."
    & cargo @arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Ghost Tap processor workflow failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}
