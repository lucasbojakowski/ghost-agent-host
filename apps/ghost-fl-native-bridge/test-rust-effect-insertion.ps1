param(
    [int]$Track = 1,
    [int]$Slot = 10,
    [string]$Plugin = 'Pro-Q 4',
    [switch]$IHaveConfirmedTheSlotIsEmpty,
    [switch]$RemoveCreatedEffectAfterTest
)

$ErrorActionPreference = 'Stop'

function Write-Step([string]$Message) {
    Write-Host "[ghost-fl-rust-insert] $Message"
}

if (-not $IHaveConfirmedTheSlotIsEmpty) {
    throw "Safety stop: confirm Mixer Insert $Track, slot $Slot is empty, then rerun with -IHaveConfirmedTheSlotIsEmpty. This test inserts '$Plugin' into that exact slot."
}

function Invoke-RustTool {
    param(
        [string]$Tool,
        [string]$ArgsJson
    )

    # Windows PowerShell 5.1 -> native argv parsing can split a JSON argument at
    # whitespace that lives inside a quoted JSON string after the quote escaping
    # needed for cargo.exe. JSON \u0020 decodes back to an ordinary space in Rust,
    # while keeping the native argv token free of literal spaces.
    $wireArgsJson = $ArgsJson.Replace(' ', '\u0020')
    $nativeArgsJson = $wireArgsJson.Replace('"', '\"')
    Write-Step "Input JSON for '$Tool' (intentionally scrambled): $ArgsJson"

    # Rust writes normal diagnostics to stderr. PowerShell 5.1 converts redirected
    # native stderr into ErrorRecord objects; with ErrorActionPreference=Stop that
    # can abort a successful cargo command. Relax it only around the native call
    # and use LASTEXITCODE as the authoritative failure signal.
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

function Escape-JsonString([string]$Text) {
    # Enough for these simple command-line smoke-test strings.
    return $Text.Replace('\', '\\').Replace('"', '\"')
}

Write-Step 'Running Rust unit tests first ...'
& cargo test -p ghost-fl-native-bridge
if ($LASTEXITCODE -ne 0) {
    throw "cargo test failed with exit code $LASTEXITCODE"
}

$pluginJson = Escape-JsonString $Plugin

Write-Step "Inserting '$Plugin' into Mixer Insert $Track, slot $Slot through Rust ..."
# Wrong order on purpose: slot_number, target_tracks, plugin.
$addJson = '{"slot_number":' + $Slot + ',"target_tracks":"' + $Track + '","plugin":"' + $pluginJson + '"}'
[void](Invoke-RustTool -Tool 'add_effect' -ArgsJson $addJson)

Start-Sleep -Milliseconds 750
Write-Step 'Resolving the inserted plugin parameter manifest through Rust ...'
# Wrong order on purpose: slot_number, target.
$listJson = '{"slot_number":' + $Slot + ',"target":"' + $Track + '"}'
$listOutput = Invoke-RustTool -Tool 'get_plugin_parameter_list' -ArgsJson $listJson

if (($listOutput -notmatch 'Parameters for') -or ($listOutput -notmatch [Regex]::Escape($Plugin))) {
    throw "add_effect returned successfully, but parameter inspection did not clearly resolve '$Plugin' at Insert $Track slot $Slot.`n$listOutput"
}

Write-Step "GREEN: Rust-native add_effect inserted and resolved '$Plugin' on Insert $Track slot $Slot."

if ($RemoveCreatedEffectAfterTest) {
    Write-Step 'Explicit cleanup requested; removing only the effect created in this confirmed-empty test slot ...'
    # remove_effect schema is target_tracks, slot_numbers. Scramble it deliberately.
    $removeJson = '{"slot_numbers":"' + $Slot + '","target_tracks":"' + $Track + '"}'
    [void](Invoke-RustTool -Tool 'remove_effect' -ArgsJson $removeJson)
    Start-Sleep -Milliseconds 500

    $verifyOutput = $null
    try {
        $verifyOutput = Invoke-RustTool -Tool 'get_plugin_parameter_list' -ArgsJson $listJson
    }
    catch {
        # Rust intentionally treats Gopher's soft "Could not resolve plugin target" response
        # as a non-zero exit. That is the expected post-removal state for an empty slot.
        $verifyOutput = $_ | Out-String
    }

    if ($verifyOutput -notmatch 'Could not resolve plugin target') {
        throw "Cleanup call returned, but the slot did not verify as empty. Inspect Insert $Track slot $Slot manually.`n$verifyOutput"
    }
    Write-Step 'GREEN: the exact effect created by this test was removed and the slot verified empty.'
}
else {
    Write-Step "Effect left in place intentionally. Remove only '$Plugin' from Insert $Track slot $Slot when finished, or rerun with -RemoveCreatedEffectAfterTest next time."
}
