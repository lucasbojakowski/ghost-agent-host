param(
    [int]$Track = 1,
    [int]$Slot = 10,
    [string]$ParamIdentifier = '558',
    [string]$ParamName = 'Output Pan',
    [int]$Port = 9222
)

$ErrorActionPreference = 'Stop'

function Write-Step([string]$Message) {
    Write-Host "[ghost-fl-value-diagnose] $Message"
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
        return "LOCAL_CALL_ERROR: $($_ | Out-String)"
    }

    return ($output -join [Environment]::NewLine)
}

function Summarize-Result {
    param(
        [string]$Label,
        [string]$Output
    )

    if ($Output -match 'Normalized Value:\s*([0-9]+(?:\.[0-9]+)?)') {
        Write-Step "$Label => VALUE $($Matches[1])"
        return
    }

    if ($Output -match "Parameters for '([^']+)'" ) {
        Write-Step "$Label => PARAMETER LIST RESOLVED plugin '$($Matches[1])'"
        return
    }

    if ($Output -match 'Error: Could not resolve plugin target[^"\r\n]*') {
        Write-Step "$Label => $($Matches[0])"
        return
    }

    if ($Output -match 'Error: Could not find parameter[^"\r\n]*') {
        Write-Step "$Label => $($Matches[0])"
        return
    }

    if ($Output -match 'Traceback[^}]*') {
        Write-Step "$Label => native traceback returned"
        Write-Host $Matches[0]
        return
    }

    Write-Step "$Label => unclassified response"
    $previewLength = [Math]::Min(700, $Output.Length)
    Write-Host $Output.Substring(0, $previewLength)
}

Write-Step "Read-only diagnostic for Mixer Insert $Track, slot $Slot. No parameter writes will be performed."

Write-Step 'Checking project/session snapshot for the target plugin slot context ...'
$session = Invoke-GopherTool -Tool 'get_session_context' -ToolArgs @{}
if ($session -match 'Pro-Q 4') {
    Write-Step "Session context contains 'Pro-Q 4'."
}
else {
    Write-Step "Session context text did not contain 'Pro-Q 4' (this is only a diagnostic hint)."
}

$numericTarget = [string]$Track
$visualNameTarget = "Insert $Track"

Write-Step "Control check: get_plugin_parameter_list with target '$numericTarget' ..."
$listNumeric = Invoke-GopherTool -Tool 'get_plugin_parameter_list' -ToolArgs @{
    target = $numericTarget
    slot_number = $Slot
}
Summarize-Result -Label "list target='$numericTarget'" -Output $listNumeric

Write-Step "Trying get_plugin_parameter_value with the exact target/index shape used by the roundtrip test ..."
$valueNumericIndex = Invoke-GopherTool -Tool 'get_plugin_parameter_value' -ToolArgs @{
    target = $numericTarget
    param_identifier = [string]$ParamIdentifier
    slot_number = $Slot
}
Summarize-Result -Label "value target='$numericTarget' param='$ParamIdentifier'" -Output $valueNumericIndex

Write-Step "Trying the same target with exact parameter name '$ParamName' ..."
$valueNumericName = Invoke-GopherTool -Tool 'get_plugin_parameter_value' -ToolArgs @{
    target = $numericTarget
    param_identifier = $ParamName
    slot_number = $Slot
}
Summarize-Result -Label "value target='$numericTarget' param='$ParamName'" -Output $valueNumericName

Write-Step "Trying visual mixer-track name '$visualNameTarget' with the parameter index ..."
$valueNameIndex = Invoke-GopherTool -Tool 'get_plugin_parameter_value' -ToolArgs @{
    target = $visualNameTarget
    param_identifier = [string]$ParamIdentifier
    slot_number = $Slot
}
Summarize-Result -Label "value target='$visualNameTarget' param='$ParamIdentifier'" -Output $valueNameIndex

Write-Step 'Diagnostic complete. Send the compact summary lines back; no state was changed.'
