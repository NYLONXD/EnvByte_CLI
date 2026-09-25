# Builds envbyte.msi, the Windows installer, around an envbyte.exe.
#
#   pwsh packaging/msi/build.ps1 -Version 0.4.2 -Binary target/release/envbyte.exe -Out envbyte.msi
#
# Needs the .NET SDK. WiX is fetched into a temporary folder the first time, so
# nothing is installed globally. Release and CI builds both run this script.

param(
    [Parameter(Mandatory)] [string] $Version,
    [Parameter(Mandatory)] [string] $Binary,
    [string] $Out = 'envbyte.msi'
)

$ErrorActionPreference = 'Stop'

# WiX 6 and later need a maintenance fee agreement accepted before they build.
$wixVersion = '5.0.2'
$wixHome = Join-Path ([IO.Path]::GetTempPath()) "envbyte-wix-$wixVersion"
$wix = Join-Path $wixHome 'wix.exe'

function Invoke-Checked {
    param([string] $What, [scriptblock] $Command)
    & $Command
    if ($LASTEXITCODE) { throw "$What failed with exit code $LASTEXITCODE." }
}

if (-not (Test-Path $wix)) {
    Invoke-Checked 'Installing WiX' { dotnet tool install wix --version $wixVersion --tool-path $wixHome }
}

# Windows Installer versions are numbers only: 0.5.0-rc.1 installs as 0.5.0.
$msiVersion = $Version.TrimStart('v') -replace '[-+].*$', ''
if ($msiVersion -notmatch '^\d+\.\d+\.\d+$') { throw "'$Version' is not a version such as 0.4.2." }

$source = Join-Path $PSScriptRoot 'envbyte.wxs'
$Binary = (Resolve-Path $Binary).Path
$Out = [IO.Path]::GetFullPath($Out, (Get-Location).Path)

# `wix extension add` caches the extension in .wix under the current folder,
# so work from WiX's own folder to keep it out of the repository.
Push-Location $wixHome
try {
    Invoke-Checked 'Fetching the WiX UI extension' { & $wix extension add "WixToolset.UI.wixext/$wixVersion" }
    Invoke-Checked 'Building envbyte.msi' {
        & $wix build $source -arch x64 -ext WixToolset.UI.wixext `
            -d "Version=$msiVersion" -d "Binary=$Binary" `
            -pdbtype none -o $Out
    }
} finally {
    Pop-Location
}

Write-Host "Built $Out (Envbyte $msiVersion)"
