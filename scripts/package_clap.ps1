[CmdletBinding()]
param(
    [string]$OutputDirectory = "",
    [switch]$Install,
    [string]$InstallDirectory = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$rustTarget = "x86_64-pc-windows-msvc"
$pluginPackage = "ghost-clap-plugin"
$pluginLibrary = "ghost_clap_plugin.dll"
$pluginFileName = "Ghost Tap.clap"
$legacyPluginFileName = "Ghost Agent.clap"

function Resolve-Cargo {
    $cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
    if ($null -ne $cargoCommand) {
        return $cargoCommand.Source
    }

    $fallback = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
    if (Test-Path -LiteralPath $fallback -PathType Leaf) {
        return $fallback
    }

    throw "Cargo was not found on PATH or at '$fallback'."
}

function Assert-X64Pe([string]$Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    $reader = [System.IO.BinaryReader]::new($stream)
    try {
        if (($reader.ReadByte() -ne 0x4d) -or ($reader.ReadByte() -ne 0x5a)) {
            throw "Plugin binary is not a PE file: $Path"
        }

        $stream.Position = 0x3c
        $peOffset = $reader.ReadInt32()
        $stream.Position = $peOffset
        if ($reader.ReadUInt32() -ne 0x00004550) {
            throw "Plugin binary has an invalid PE signature: $Path"
        }

        $machine = $reader.ReadUInt16()
        if ($machine -ne 0x8664) {
            throw ("Plugin binary is not Windows x64 (PE machine 0x{0:X4}): {1}" -f $machine, $Path)
        }
    }
    finally {
        $reader.Dispose()
    }
}

function Assert-ClapEntry([string]$Path) {
    if ($null -eq ("GhostClapPackaging.NativeMethods" -as [type])) {
        Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

namespace GhostClapPackaging {
    public static class NativeMethods {
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern IntPtr LoadLibraryEx(string fileName, IntPtr reserved, uint flags);

        [DllImport("kernel32.dll", CharSet = CharSet.Ansi, ExactSpelling = true, SetLastError = true)]
        public static extern IntPtr GetProcAddress(IntPtr module, string procedureName);

        [DllImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool FreeLibrary(IntPtr module);
    }
}
"@
    }

    $loadWithAlteredSearchPath = 0x00000008
    $module = [GhostClapPackaging.NativeMethods]::LoadLibraryEx($Path, [IntPtr]::Zero, $loadWithAlteredSearchPath)
    if ($module -eq [IntPtr]::Zero) {
        $errorCode = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
        $message = [ComponentModel.Win32Exception]::new($errorCode).Message
        throw "Windows could not load the plugin binary ($errorCode): $message"
    }

    try {
        $entry = [GhostClapPackaging.NativeMethods]::GetProcAddress($module, "clap_entry")
        if ($entry -eq [IntPtr]::Zero) {
            throw "Plugin binary does not export the required CLAP symbol 'clap_entry'."
        }
    }
    finally {
        [void][GhostClapPackaging.NativeMethods]::FreeLibrary($module)
    }
}

$cargo = Resolve-Cargo
Push-Location $repositoryRoot
try {
    # Ghost Tap intentionally changes workspace-local dependency edges during the local validation
    # cycle. Let Cargo refresh Cargo.lock; we commit the settled lockfile after the runtime gate.
    $metadataJson = & $cargo metadata --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) {
        throw "cargo metadata failed with exit code $LASTEXITCODE."
    }

    $metadata = $metadataJson | ConvertFrom-Json
    $package = $metadata.packages | Where-Object { $_.name -eq $pluginPackage } | Select-Object -First 1
    if ($null -eq $package) {
        throw "Cargo package '$pluginPackage' was not found in the workspace."
    }

    & $cargo build --release --package $pluginPackage --target $rustTarget
    if ($LASTEXITCODE -ne 0) {
        throw "Ghost Tap CLAP release build failed with exit code $LASTEXITCODE."
    }

    $libraryPath = Join-Path $metadata.target_directory "$rustTarget\release\$pluginLibrary"
    if (-not (Test-Path -LiteralPath $libraryPath -PathType Leaf)) {
        throw "Expected plugin library was not produced: $libraryPath"
    }

    Assert-X64Pe $libraryPath
    Assert-ClapEntry $libraryPath

    if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
        $OutputDirectory = Join-Path $repositoryRoot "dist\windows-x86_64"
    }
    elseif (-not [System.IO.Path]::IsPathRooted($OutputDirectory)) {
        $OutputDirectory = Join-Path $repositoryRoot $OutputDirectory
    }

    $outputPath = [System.IO.Path]::GetFullPath($OutputDirectory)
    New-Item -ItemType Directory -Force -Path $outputPath | Out-Null

    $clapPath = Join-Path $outputPath $pluginFileName
    Copy-Item -LiteralPath $libraryPath -Destination $clapPath -Force

    $hash = Get-FileHash -LiteralPath $clapPath -Algorithm SHA256
    $checksumPath = "$clapPath.sha256"
    $checksumLine = "{0} *{1}{2}" -f $hash.Hash.ToLowerInvariant(), $pluginFileName, [Environment]::NewLine
    [System.IO.File]::WriteAllText($checksumPath, $checksumLine, [System.Text.Encoding]::ASCII)

    $archiveName = "ghost-tap-{0}-windows-x86_64.zip" -f $package.version
    $archivePath = Join-Path $outputPath $archiveName
    Compress-Archive -LiteralPath $clapPath, $checksumPath -DestinationPath $archivePath -Force

    $installedPath = $null
    $removedLegacyPath = $null
    if ($Install) {
        if ([string]::IsNullOrWhiteSpace($InstallDirectory)) {
            $InstallDirectory = Join-Path $env:CommonProgramFiles "CLAP"
        }
        elseif (-not [System.IO.Path]::IsPathRooted($InstallDirectory)) {
            throw "InstallDirectory must be an absolute path."
        }

        New-Item -ItemType Directory -Force -Path $InstallDirectory | Out-Null
        $legacyPath = Join-Path $InstallDirectory $legacyPluginFileName
        if (Test-Path -LiteralPath $legacyPath -PathType Leaf) {
            Remove-Item -LiteralPath $legacyPath -Force
            $removedLegacyPath = $legacyPath
        }
        $installedPath = Join-Path $InstallDirectory $pluginFileName
        Copy-Item -LiteralPath $clapPath -Destination $installedPath -Force
    }

    Write-Host "Ghost Tap CLAP: $clapPath"
    Write-Host "SHA-256:       $($hash.Hash.ToLowerInvariant())"
    Write-Host "Archive:       $archivePath"
    if ($null -ne $removedLegacyPath) {
        Write-Host "Removed legacy: $removedLegacyPath"
    }
    if ($null -ne $installedPath) {
        Write-Host "Installed:      $installedPath"
    }
}
finally {
    Pop-Location
}
