param(
    [int]$Track = 1,
    [int]$Slot = 10,
    [string]$Plugin = 'Fruity Reeverb 2',
    [int]$Port = 9222,
    [switch]$IHaveConfirmedTheSlotIsEmpty,
    [switch]$ForceKnownBrokenAddEffect
)

$ErrorActionPreference = 'Stop'

function Write-Step([string]$Message) {
    Write-Host "[ghost-fl-insert-test] $Message"
}

if (-not $IHaveConfirmedTheSlotIsEmpty) {
    throw "Safety stop: confirm Mixer Insert $Track, slot $Slot is empty, then rerun with -IHaveConfirmedTheSlotIsEmpty. This test deliberately inserts a plugin into that exact slot."
}

if (-not $ForceKnownBrokenAddEffect) {
    throw "Known FL Studio 26.1.3 Gopher bug: MCPTools.py add_effect line 1480 computes slot_number - 1, but the Gopher bridge is delivering slot_number as a string. The uploaded MCPTools.pyc confirms target_tracks is resolved separately and is expected to remain a string. This test is disabled by default until FL changes or we find a lower-level route. Use -ForceKnownBrokenAddEffect only to deliberately reproduce the native failure."
}

$caller = Join-Path $PSScriptRoot 'call-gopher-tool.ps1'
if (-not (Test-Path $caller)) {
    throw "Missing helper: $caller"
}

function Invoke-GopherTool {
    param(
        [string]$Tool,
        [hashtable]$ToolArgs
    )

    $json = $ToolArgs | ConvertTo-Json -Compress -Depth 20

    try {
        $output = & $caller `
            -Port $Port `
            -Tool $Tool `
            -ArgsJson $json 2>&1
    }
    catch {
        throw "Tool '$Tool' failed:`n$($_ | Out-String)"
    }

    return ($output -join [Environment]::NewLine)
}

Write-Step "Advisory preflight: checking '$Plugin' against FL Studio's Plugin database ..."
$browser = Invoke-GopherTool -Tool 'get_browser_names' -ToolArgs @{
    name = 'Plugin database'
    fullRecursive = 1
}

$browserText = [string]$browser
$escaped = [Regex]::Escape($Plugin)
if ($browserText -match $escaped) {
    Write-Step "Plugin database text contains '$Plugin'."
}
else {
    Write-Step "Plugin database text matcher did not find '$Plugin'; continuing so FL's add_effect tool can validate the exact base name directly."
}

Write-Step "Reproducing known add_effect failure for '$Plugin' on Mixer Insert $Track, slot $Slot ..."
$add = Invoke-GopherTool -Tool 'add_effect' -ToolArgs @{
    plugin = $Plugin
    target_tracks = [string]$Track
    slot_number = $Slot
}
Write-Host $add

Write-Step 'Known-broken reproduction completed. No parameter inspection is attempted unless add_effect is fixed by FL Studio.'
