param(
    [int]$Track = 1,
    [int]$Slot = 10,
    [string]$Plugin = 'Fruity Reeverb 2',
    [int]$Port = 9222,
    [switch]$IHaveConfirmedTheSlotIsEmpty
)

$ErrorActionPreference = 'Stop'

function Write-Step([string]$Message) {
    Write-Host "[ghost-fl-insert-test] $Message"
}

if (-not $IHaveConfirmedTheSlotIsEmpty) {
    throw "Safety stop: confirm Mixer Insert $Track, slot $Slot is empty, then rerun with -IHaveConfirmedTheSlotIsEmpty. This test deliberately inserts a plugin into that exact slot."
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

Write-Step "Inserting '$Plugin' into Mixer Insert $Track, slot $Slot ..."
Write-Step "Diagnostic note: FL 26.1.3's published add_effect schema says target_tracks is a string, but the previous native traceback attempted string-minus-integer. This run intentionally sends the track as a JSON integer to test the implementation behind the schema."
$add = Invoke-GopherTool -Tool 'add_effect' -ToolArgs @{
    plugin = $Plugin
    target_tracks = $Track
    slot_number = $Slot
}
Write-Host $add

Start-Sleep -Milliseconds 750

Write-Step 'Reading the inserted plugin parameter manifest ...'
$params = Invoke-GopherTool -Tool 'get_plugin_parameter_list' -ToolArgs @{
    target = [string]$Track
    slot_number = $Slot
}
Write-Host $params

Write-Step 'Insertion + inspection stage completed.'
Write-Step "Verify visually that '$Plugin' appeared on Mixer Insert $Track, slot $Slot."
Write-Step 'Do not remove it yet; send the parameter-list output back so the next test can choose one harmless parameter for write/readback.'
Write-Step "If you need to clean up manually, remove only the plugin created by this test from Insert $Track, slot $Slot."
