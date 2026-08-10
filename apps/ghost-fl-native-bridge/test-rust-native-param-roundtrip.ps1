param(
    [int]$Track = 1,
    [int]$Slot = 10,
    [string]$ParamIdentifier = '558',
    [double]$Delta = 0.01,
    [switch]$IHaveStoppedTransportAndAcceptedTemporaryParameterChange
)

$ErrorActionPreference = 'Stop'

function Write-Step([string]$Message) {
    Write-Host "[ghost-fl-rust-roundtrip] $Message"
}

if (-not $IHaveStoppedTransportAndAcceptedTemporaryParameterChange) {
    throw "Safety stop: stop transport, use a scratch plugin instance, then rerun with -IHaveStoppedTransportAndAcceptedTemporaryParameterChange. This test temporarily changes one plugin parameter and restores it."
}

function Invoke-RustTool {
    param(
        [string]$Tool,
        [string]$ArgsJson
    )

    # Windows PowerShell 5.1 consumes quotes when forwarding JSON through cargo.exe.
    # Escape only the native-process boundary; Rust still receives the same object.
    $nativeArgsJson = $ArgsJson.Replace('"', '\"')
    Write-Step "Input JSON for '$Tool' (intentionally scrambled): $ArgsJson"

    # Rust intentionally writes diagnostics (target attach, canonical argument order)
    # to stderr with eprintln!. In Windows PowerShell 5.1, `2>&1` turns native stderr
    # records into PowerShell ErrorRecord objects, and ErrorActionPreference=Stop can
    # incorrectly terminate the script even when cargo exits 0. Relax error handling
    # only for this native invocation; the real success/failure gate is LASTEXITCODE.
    $previousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = & cargo run --quiet -p ghost-fl-native-bridge -- call $Tool --args $nativeArgsJson 2>&1
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }

    $text = $output | ForEach-Object { $_.ToString() } | Out-String
    $text = $text.TrimEnd()
    Write-Host $text

    if ($exitCode -ne 0) {
        throw "Rust tool '$Tool' failed with exit code $exitCode.`n$text"
    }

    return $text
}

function Get-NormalizedValue([string]$Text) {
    $match = [Regex]::Match($Text, 'Normalized Value:\s*([0-9]+(?:\.[0-9]+)?)')
    if (-not $match.Success) {
        throw "Could not parse a normalized value from Rust native output:`n$Text"
    }
    return [double]::Parse($match.Groups[1].Value, [System.Globalization.CultureInfo]::InvariantCulture)
}

function Read-ParameterValue {
    # Wrong order on purpose: slot_number, param_identifier, target.
    $json = '{"slot_number":' + $Slot + ',"param_identifier":"' + $ParamIdentifier + '","target":"' + $Track + '"}'
    return Get-NormalizedValue (Invoke-RustTool -Tool 'get_plugin_parameter_value' -ArgsJson $json)
}

function Set-ParameterValue([double]$Value) {
    $valueText = $Value.ToString('R', [System.Globalization.CultureInfo]::InvariantCulture)
    # Wrong order on purpose: slot_number, value, param_identifier, target.
    $json = '{"slot_number":' + $Slot + ',"value":' + $valueText + ',"param_identifier":"' + $ParamIdentifier + '","target":"' + $Track + '"}'
    [void](Invoke-RustTool -Tool 'set_plugin_parameter_value' -ArgsJson $json)
}

Write-Step 'Running Rust unit tests first ...'
& cargo test -p ghost-fl-native-bridge
if ($LASTEXITCODE -ne 0) {
    throw "cargo test failed with exit code $LASTEXITCODE"
}

Write-Step "Target: Mixer Insert $Track, slot $Slot, parameter $ParamIdentifier."
Write-Step 'Reading original value through Rust ...'
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

$writeSucceeded = $false
$restorationVerified = $false
try {
    Write-Step ("Temporarily writing {0:F4} through Rust ..." -f $testValue)
    Set-ParameterValue $testValue
    $writeSucceeded = $true

    Start-Sleep -Milliseconds 250
    Write-Step 'Reading temporary value back through Rust ...'
    $afterWrite = Read-ParameterValue
    Write-Step ("Readback normalized value: {0:F4}" -f $afterWrite)

    if ([Math]::Abs($afterWrite - $testValue) -gt 0.002) {
        throw "Temporary readback mismatch. Expected approximately $testValue, got $afterWrite."
    }
    Write-Step 'Temporary write/readback matched.'
}
finally {
    if ($writeSucceeded) {
        Write-Step ("Restoring original value {0:F4} through Rust ..." -f $original)
        Set-ParameterValue $original
        Start-Sleep -Milliseconds 250
        $restored = Read-ParameterValue
        Write-Step ("Restored normalized value: {0:F4}" -f $restored)
        if ([Math]::Abs($restored - $original) -gt 0.002) {
            throw "Restoration verification failed. Expected approximately $original, got $restored. Inspect the plugin manually."
        }
        $restorationVerified = $true
    }
}

if (-not $restorationVerified) {
    throw 'Rust mutation test did not reach verified restoration.'
}

Write-Step 'GREEN: Rust-native write -> verify -> restore completed with schema-driven argument canonicalization.'
