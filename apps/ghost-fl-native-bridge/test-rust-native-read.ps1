param(
    [int]$Track = 1,
    [int]$Slot = 10,
    [string]$ParamIdentifier = '558'
)

$ErrorActionPreference = 'Stop'

function Write-Step([string]$Message) {
    Write-Host "[ghost-fl-rust-read] $Message"
}

Write-Step 'Running Rust unit tests first ...'
& cargo test -p ghost-fl-native-bridge
if ($LASTEXITCODE -ne 0) {
    throw "cargo test failed with exit code $LASTEXITCODE"
}

# Deliberately provide the JSON keys in the WRONG order. The Rust bridge should
# fetch the live MCP schema and emit: target, param_identifier, slot_number.
$argsJson = '{"slot_number":' + $Slot + ',"param_identifier":"' + $ParamIdentifier + '","target":"' + $Track + '"}'

# Windows PowerShell 5.1 -> native .exe argument parsing consumes the JSON quote
# characters when a string is forwarded through cargo.exe. Prefix each quote
# with a backslash so cargo receives literal quotes and can pass valid JSON to
# ghost-fl-native-bridge.exe. This is only a shell-boundary quoting concern; the
# Rust bridge still receives the intentionally scrambled JSON object unchanged.
$nativeArgsJson = $argsJson.Replace('"', '\"')

Write-Step "Calling get_plugin_parameter_value with intentionally scrambled input JSON: $argsJson"
& cargo run -p ghost-fl-native-bridge -- call get_plugin_parameter_value --args $nativeArgsJson
if ($LASTEXITCODE -ne 0) {
    throw "Rust native read failed with exit code $LASTEXITCODE"
}

Write-Step 'Rust read completed. Confirm the log reports canonical order: target, param_identifier, slot_number.'
