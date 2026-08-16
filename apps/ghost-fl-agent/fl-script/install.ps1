[CmdletBinding()]
param(
    [string]$HardwareRoot = (Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'Image-Line\FL Studio\Settings\Hardware'),
    [string]$ScriptFolder = 'Ghost Bridge',
    [string]$SharedPythonLib = 'D:\Image-Line\FL Studio 2026\Shared\Python\Lib'
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

$nativeRoot = Join-Path (Split-Path -Parent $PSScriptRoot) 'fl-native'
$nativeArtifact = Get-ChildItem -LiteralPath $nativeRoot -Filter 'ghost_native*.pyd' -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

if ($null -eq $nativeArtifact) {
    Write-Warning "ghost_native .pyd was not found. Build it first with: $nativeRoot\build.ps1"
    return
}

if ([string]::IsNullOrWhiteSpace($SharedPythonLib)) {
    Write-Warning 'Native transport was built but not installed. Pass -SharedPythonLib <FL Studio\Shared\Python\Lib> or set GHOST_FL_SHARED_PYTHON_LIB.'
    return
}

New-Item -ItemType Directory -Path $SharedPythonLib -Force | Out-Null
$nativeDestination = Join-Path $SharedPythonLib $nativeArtifact.Name
Copy-Item -LiteralPath $nativeArtifact.FullName -Destination $nativeDestination -Force
Write-Host "Installed Ghost native transport to: $nativeDestination"
Write-Host 'Close FL Studio before replacing a loaded .pyd, then restart FL Studio.'
