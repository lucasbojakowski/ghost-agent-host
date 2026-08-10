param(
    [int]$Track = 1,
    [int]$Slot = 10,
    [string]$ParamIdentifier = '558',
    [double]$Delta = 0.01,
    [int]$Port = 9222,
    [switch]$IHaveStoppedTransportAndAcceptedTemporaryParameterChange
)

$ErrorActionPreference = 'Stop'

function Write-Step([string]$Message) {
    Write-Host "[ghost-fl-param-roundtrip] $Message"
}

if (-not $IHaveStoppedTransportAndAcceptedTemporaryParameterChange) {
    throw "Safety stop: stop transport, use a scratch plugin instance, then rerun with -IHaveStoppedTransportAndAcceptedTemporaryParameterChange. This test temporarily changes one normalized plugin parameter and restores the original value."
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

    # FL Studio 26.1.3's Gopher dispatcher is order-sensitive even though MCP
    # represents arguments as named JSON properties. Always serialize arguments
    # in the exact function-signature order discovered from the live tool schema.
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

    if (($Output -match '"isError"\s*:\s*true') -or
        ($Output -match 'Traceback') -or
        ($Output -match 'Error: Could not resolve plugin target')) {
        throw "Native tool '$Tool' returned an error:`n$Output"
    }
}

function Get-NormalizedValue([string]$Output) {
    Assert-NoNativeError -Tool 'get_plugin_parameter_value' -Output $Output
    $match = [Regex]::Match($Output, 'Normalized Value:\s*([0-9]+(?:\.[0-9]+)?)')
    if (-not $match.Success) {
        throw "Could not parse a normalized value from get_plugin_parameter_value output:`n$Output"
    }
    return [double]::Parse($match.Groups[1].Value, [System.Globalization.CultureInfo]::InvariantCulture)
}

function Read-ParameterValue {
    $output = Invoke-GopherTool -Tool 'get_plugin_parameter_value' -ToolArgs ([ordered]@{
        target = [string]$Track
        param_identifier = [string]$ParamIdentifier
        slot_number = $Slot
    })
    Write-Host $output
    return Get-NormalizedValue $output
}

Write-Step "Target: Mixer Insert $Track, slot $Slot, visual parameter $ParamIdentifier."
Write-Step 'For Pro-Q 4, the default parameter 558 is Output Pan.'

Write-Step 'Reading original value ...'
$original = Read-ParameterValue
Write-Step ("Original normalized value: {0:F4}" -f $original)

if ($original -le (1.0 - $Delta)) {
    $testValue = $original + $Delta
}
else {
    $testValue = $original - $Delta
}
$testValue = [Math]::Max(0.0, [Math]::Min(1.0, $testValue))

if ([Math]::Abs($testValue - $original) -lt 0.000001) {
    throw 'Unable to choose a distinct temporary test value.'
}

Write-Step ("Temporarily writing {0:F4} ..." -f $testValue)
$setOutput = Invoke-GopherTool -Tool 'set_plugin_parameter_value' -ToolArgs ([ordered]@{
    target = [string]$Track
    param_identifier = [string]$ParamIdentifier
    value = $testValue
    slot_number = $Slot
})
Write-Host $setOutput
Assert-NoNativeError -Tool 'set_plugin_parameter_value' -Output $setOutput

Start-Sleep -Milliseconds 250
Write-Step 'Reading value back ...'
$afterWrite = Read-ParameterValue
Write-Step ("Readback normalized value: {0:F4}" -f $afterWrite)

if ([Math]::Abs($afterWrite - $testValue) -gt 0.002) {
    Write-Step 'WARNING: readback did not match the requested temporary value closely.'
}
else {
    Write-Step 'Temporary write/readback matched.'
}

Write-Step ("Restoring original value {0:F4} ..." -f $original)
$restoreOutput = Invoke-GopherTool -Tool 'set_plugin_parameter_value' -ToolArgs ([ordered]@{
    target = [string]$Track
    param_identifier = [string]$ParamIdentifier
    value = $original
    slot_number = $Slot
})
Write-Host $restoreOutput
Assert-NoNativeError -Tool 'set_plugin_parameter_value' -Output $restoreOutput

Start-Sleep -Milliseconds 250
Write-Step 'Verifying restoration ...'
$restored = Read-ParameterValue
Write-Step ("Restored normalized value: {0:F4}" -f $restored)

if ([Math]::Abs($restored - $original) -gt 0.002) {
    throw "Restoration verification failed. Expected approximately $original, got $restored. Inspect the plugin manually."
}

Write-Step 'Parameter roundtrip completed and original value was restored.'
