[CmdletBinding()]
param(
    [string]$HardwareRoot = (Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'Image-Line\FL Studio\Settings\Hardware'),
    [string]$ScriptFolder = 'Ghost Bridge'
)

$ErrorActionPreference = 'Stop'

$source = Join-Path $PSScriptRoot 'device_Ghost.py'
if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
    throw "Bundled FL Studio script was not found at '$source'."
}

if ([string]::IsNullOrWhiteSpace($HardwareRoot)) {
    throw 'HardwareRoot must point to the FL Studio user-data Settings\Hardware directory.'
}

$destination = Join-Path $HardwareRoot $ScriptFolder
New-Item -ItemType Directory -Path $destination -Force | Out-Null
Copy-Item -LiteralPath $source -Destination (Join-Path $destination 'device_Ghost.py') -Force

Write-Host "Installed Ghost Bridge FL Studio script to: $destination"
Write-Host "Expected virtual MIDI bootstrap device: Ghost Midi"
Write-Host "RPC transport remains loopback TCP; loopMIDI is not used as the data plane."
