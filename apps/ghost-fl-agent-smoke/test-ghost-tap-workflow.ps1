[CmdletBinding()]
param(
    [int]$TapInstance = 0,
    [double]$CaptureSeconds = 4.0,
    [string]$Track = "1",
    [int]$SlotStart = 1,
    [int]$SlotEnd = 4,
    [string[]]$Plugin = @("Pro-Q 4", "Pro-C 3"),
    [string]$Intent = "Build a clearly audible but musical processor chain for this sample from the measured evidence. Improve clarity, balance, dynamics, and usefulness in a mix while preserving the sample's identity.",
    [double]$ProcessingIntensity = 0.70,
    [string]$Model = "gpt-5.6-terra",
    [string]$CodexBinary = "codex",
    [int]$DebugPort = 9222,
    [switch]$VerboseAgentEvents,
    [switch]$IHavePositionedPlayheadAndAcceptedScopedWrites,
    [switch]$IHavePositionedPlayheadAndConfirmedEmptySlots
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$acceptedWrites = $IHavePositionedPlayheadAndAcceptedScopedWrites -or $IHavePositionedPlayheadAndConfirmedEmptySlots
if (-not $acceptedWrites) {
    throw "Safety acknowledgement required. Position the playhead immediately before the sample, stop transport, confirm the target mixer track/write range, then pass -IHavePositionedPlayheadAndAcceptedScopedWrites. Ghost now live-checks slot occupancy and refuses to overwrite existing effects."
}
if ($ProcessingIntensity -lt 0.0 -or $ProcessingIntensity -gt 1.0) {
    throw "ProcessingIntensity must be in 0..1."
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
        "--processing-intensity", $ProcessingIntensity.ToString([Globalization.CultureInfo]::InvariantCulture),
        "--model", $Model,
        "--codex-binary", $CodexBinary,
        "--i-have-positioned-playhead-and-accepted-scoped-writes"
    )
    if ($VerboseAgentEvents) {
        $arguments += "--verbose-agent-events"
    }
    foreach ($name in $Plugin) {
        $arguments += "--plugin"
        $arguments += $name
    }

    Write-Host "[ghost-tap-workflow] Running capture -> analysis -> Codex processor thread -> FL chain workflow at intensity $ProcessingIntensity ..."
    & cargo @arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Ghost Tap processor workflow failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}
