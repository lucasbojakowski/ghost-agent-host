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
        [System.Collections.IDictionary]$ToolArgs
    )

    # FL Studio 26.1.3's Gopher dispatcher is order-sensitive. Serialize every
    # tool call in the exact function-signature order from the live MCP schema.
    $json = $ToolArgs | ConvertTo-Json -Compress -Depth 20
    $keyOrder = @($ToolArgs.Keys) -join ', '
    Write-Step "RPC arg order for '$Tool': $keyOrder"

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

function Assert-NoNativeError {
    param(
        [string]$Tool,
        [string]$Output
    )

    if (($Output -match '"isError"\s*:\s*true') -or ($Output -match 'Traceback')) {
        throw "Native tool '$Tool' returned an error:`n$Output"
    }
}

Write-Step "Advisory preflight: checking '$Plugin' against FL Studio's Plugin database ..."
$browser = Invoke-GopherTool -Tool 'get_browser_names' -ToolArgs ([ordered]@{
    name = 'Plugin database'
    fullRecursive = 1
})

$browserText = [string]$browser
$escaped = [Regex]::Escape($Plugin)
if ($browserText -match $escaped) {
    Write-Step "Plugin database text contains '$Plugin'."
}
else {
    Write-Step "Plugin database text matcher did not find '$Plugin'; continuing so FL's add_effect tool can validate the exact base name directly."
}

Write-Step "Inserting '$Plugin' into Mixer Insert $Track, slot $Slot ..."
$add = Invoke-GopherTool -Tool 'add_effect' -ToolArgs ([ordered]@{
    plugin = $Plugin
    target_tracks = [string]$Track
    slot_number = $Slot
})
Write-Host $add
Assert-NoNativeError -Tool 'add_effect' -Output $add

Start-Sleep -Milliseconds 750

Write-Step 'Reading the inserted plugin parameter manifest ...'
$params = Invoke-GopherTool -Tool 'get_plugin_parameter_list' -ToolArgs ([ordered]@{
    target = [string]$Track
    slot_number = $Slot
})
Write-Host $params
Assert-NoNativeError -Tool 'get_plugin_parameter_list' -Output $params

if ($params -match 'Error: Could not resolve plugin target') {
    throw "Insertion returned without a native traceback, but the inserted plugin could not be resolved at Insert $Track slot $Slot. Inspect FL Studio before continuing."
}

Write-Step 'Insertion + inspection stage completed.'
Write-Step "Verify visually that '$Plugin' appeared on Mixer Insert $Track, slot $Slot."
Write-Step 'Do not remove it yet; if insertion succeeded, we can use the same ordered-call invariant for parameter mutation and controlled cleanup.'
Write-Step "If you need to clean up manually, remove only the plugin created by this test from Insert $Track, slot $Slot."
