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

    # Invoke the helper script in the current PowerShell process. Starting a new
    # powershell.exe and passing JSON on its command line lets Windows/PowerShell
    # strip the embedded JSON quotes before ArgsJson reaches the child process.
    # Script invocation preserves the JSON string exactly.
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

# The browser tool returns a large nested text payload. Matching against the
# pretty-printed wrapper is useful as a hint, but it is not reliable enough to
# gate the mutation. The add_effect tool is the authoritative validator.
$browserText = [string]$browser
$escaped = [Regex]::Escape($Plugin)
if ($browserText -match $escaped) {
    Write-Step "Plugin database text contains '$Plugin'."
}
else {
    Write-Step "Plugin database text matcher did not find '$Plugin'; continuing so FL's add_effect tool can validate the exact base name directly."
}

Write-Step "Inserting '$Plugin' into Mixer Insert $Track, slot $Slot ..."
Write-Step "Use the exact base plugin name only (for example 'Fruity Reeverb 2'), not a .fst suffix or browser path."
$add = Invoke-GopherTool -Tool 'add_effect' -ToolArgs @{
    plugin = $Plugin
    target_tracks = [string]$Track
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
