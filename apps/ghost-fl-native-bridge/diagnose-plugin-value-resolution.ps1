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
        [System.Collections.IDictionary]$ToolArgs
    )

    # IMPORTANT: Gopher/script_handler appears sensitive to JSON argument order.
    # Use OrderedDictionary so ConvertTo-Json preserves the MCP function signature
    # order instead of relying on PowerShell Hashtable enumeration order.
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

    # ConvertTo-Json escapes apostrophes as \u0027 in this environment, so accept
    # either representation when recognizing the successful parameter-list text.
    if (($Output -match "Parameters for '([^']+)'") -or ($Output -match 'Parameters for \\u0027([^\\]+)\\u0027')) {
        Write-Step "$Label => PARAMETER LIST RESOLVED"
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

Write-Step "Read-only ordered-argument diagnostic for Mixer Insert $Track, slot $Slot. No parameter writes will be performed."
Write-Step 'Hypothesis: script_handler may bind tool arguments positionally using JSON property order; ordinary PowerShell hashtables do not give us a reliable signature order.'

$numericTarget = [string]$Track

Write-Step "Control check: parameter list with signature order target, slot_number ..."
$listNumeric = Invoke-GopherTool -Tool 'get_plugin_parameter_list' -ToolArgs ([ordered]@{
    target = $numericTarget
    slot_number = $Slot
})
Summarize-Result -Label "ordered list target='$numericTarget'" -Output $listNumeric

Write-Step "Testing value lookup with exact function signature order target, param_identifier, slot_number ..."
$valueNumericIndex = Invoke-GopherTool -Tool 'get_plugin_parameter_value' -ToolArgs ([ordered]@{
    target = $numericTarget
    param_identifier = [string]$ParamIdentifier
    slot_number = $Slot
})
Summarize-Result -Label "ordered value target='$numericTarget' param='$ParamIdentifier'" -Output $valueNumericIndex

Write-Step "Testing exact parameter name with the same signature order ..."
$valueNumericName = Invoke-GopherTool -Tool 'get_plugin_parameter_value' -ToolArgs ([ordered]@{
    target = $numericTarget
    param_identifier = $ParamName
    slot_number = $Slot
})
Summarize-Result -Label "ordered value target='$numericTarget' param='$ParamName'" -Output $valueNumericName

Write-Step 'Diagnostic complete. If either ordered value call returns VALUE, argument ordering is the missing bridge invariant.'
