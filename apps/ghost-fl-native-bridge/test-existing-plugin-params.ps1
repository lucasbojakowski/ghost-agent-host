param(
    [int]$Track = 1,
    [int]$Slot = 10,
    [int]$Port = 9222
)

$ErrorActionPreference = 'Stop'

function Write-Step([string]$Message) {
    Write-Host "[ghost-fl-param-test] $Message"
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

Write-Step "Inspecting existing plugin on Mixer Insert $Track, slot $Slot ..."
Write-Step "Load the plugin manually in FL Studio before running this probe."

$params = Invoke-GopherTool -Tool 'get_plugin_parameter_list' -ToolArgs @{
    target = [string]$Track
    slot_number = $Slot
}

Write-Host $params
Write-Step 'Parameter inspection completed. Send the output back before we perform any parameter write/readback.'
