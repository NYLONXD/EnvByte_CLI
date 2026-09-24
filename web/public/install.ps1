# Envbyte installer for Windows.
#
#   irm https://envbyte.trackedge.in/install.ps1 | iex
#
# Downloads the release archive from GitHub, checks it against the SHA-256
# published with the release, installs envbyte.exe into ~\.envbyte\bin and adds
# that folder to your user PATH. No administrator rights are needed.
#
# Settings (environment variables):
#   $env:ENVBYTE_VERSION = '0.4.0'     install that version instead of the latest
#   $env:ENVBYTE_INSTALL_DIR = '<dir>' install somewhere other than ~\.envbyte\bin
#   $env:ENVBYTE_NO_MODIFY_PATH = '1'  leave PATH alone
#
# The whole installer is one script block invoked on the last line, so a
# download cut off halfway runs nothing, and none of its variables leak into
# your session.

& {
    $ErrorActionPreference = 'Stop'
    # Windows PowerShell 5.1 renders the progress bar so slowly that it
    # dominates download time.
    $ProgressPreference = 'SilentlyContinue'
    [Net.ServicePointManager]::SecurityProtocol =
        [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $repo = 'NYLONXD/EnvByte_CLI'
    $target = 'x86_64-pc-windows-msvc'

    function Fail([string] $message) {
        Write-Host "error: $message" -ForegroundColor Red
        throw $message
    }

    $arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
    switch ($arch) {
        'AMD64' { }
        # Windows 11 on ARM runs x64 programs through built-in emulation.
        'ARM64' { Write-Host 'ARM64 detected: installing the x64 build, which Windows runs under emulation.' }
        default { Fail "unsupported architecture '$arch'. Envbyte needs 64-bit Windows." }
    }

    $version = "$env:ENVBYTE_VERSION".TrimStart('v')
    $base = if ($version) {
        "https://github.com/$repo/releases/download/v$version"
    } else {
        "https://github.com/$repo/releases/latest/download"
    }
    $installDir = if ($env:ENVBYTE_INSTALL_DIR) { $env:ENVBYTE_INSTALL_DIR } else { Join-Path $HOME '.envbyte\bin' }
    $archive = "envbyte-$target.zip"

    $scratch = Join-Path ([IO.Path]::GetTempPath()) ("envbyte-" + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $scratch | Out-Null
    try {
        $label = if ($version) { $version } else { '(latest)' }
        Write-Host "Downloading envbyte $label for $target"
        try {
            Invoke-WebRequest -UseBasicParsing -Uri "$base/$archive" -OutFile (Join-Path $scratch $archive)
            Invoke-WebRequest -UseBasicParsing -Uri "$base/$archive.sha256" -OutFile (Join-Path $scratch "$archive.sha256")
        } catch {
            Fail "could not download $base/$archive ($($_.Exception.Message))"
        }

        # A secrets tool must not install a binary it could not verify.
        $expected = ((Get-Content (Join-Path $scratch "$archive.sha256") -Raw).Trim() -split '\s+')[0]
        # .NET directly rather than Get-FileHash / Expand-Archive: those live in
        # modules that fail to load when Windows PowerShell 5.1 is started from
        # PowerShell 7 with its module path.
        $stream = [IO.File]::OpenRead((Join-Path $scratch $archive))
        try {
            $sha256 = [Security.Cryptography.SHA256]::Create()
            $actual = [BitConverter]::ToString($sha256.ComputeHash($stream)) -replace '-', ''
        } finally {
            $stream.Dispose()
        }
        if (-not $expected -or $expected -ne $actual) {
            Fail "checksum mismatch for ${archive}: expected $expected, got $actual"
        }
        Write-Host 'Checksum verified'

        Add-Type -AssemblyName System.IO.Compression.FileSystem
        [IO.Compression.ZipFile]::ExtractToDirectory((Join-Path $scratch $archive), $scratch)
        $binary = Join-Path $scratch "envbyte-$target\envbyte.exe"
        if (-not (Test-Path $binary)) { Fail "the archive does not contain envbyte-$target\envbyte.exe" }

        New-Item -ItemType Directory -Force -Path $installDir | Out-Null
        $destination = Join-Path $installDir 'envbyte.exe'
        # A running .exe cannot be overwritten, but it can be renamed; moving it
        # aside lets an upgrade succeed while envbyte is in use.
        if (Test-Path $destination) {
            $previous = "$destination.old"
            Remove-Item $previous -Force -ErrorAction SilentlyContinue
            Move-Item $destination $previous -Force
        }
        Copy-Item $binary $destination
    } finally {
        Remove-Item $scratch -Recurse -Force -ErrorAction SilentlyContinue
    }

    $installed = & $destination --version
    if (-not $installed) { Fail "installed $destination, but it did not run" }
    Write-Host "Installed $installed to $destination" -ForegroundColor Green

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = @($userPath -split ';' | Where-Object { $_ })
    if ($entries -notcontains $installDir) {
        if ($env:ENVBYTE_NO_MODIFY_PATH -eq '1') {
            Write-Host "Add $installDir to your PATH to run envbyte from anywhere."
        } else {
            [Environment]::SetEnvironmentVariable('Path', (@($installDir) + $entries) -join ';', 'User')
            Write-Host "Added $installDir to your user PATH."
        }
    }
    # Usable in this window straight away, not only in new ones.
    if (($env:Path -split ';') -notcontains $installDir) {
        $env:Path = "$installDir;$env:Path"
    }

    Write-Host ''
    Write-Host 'Get started:'
    Write-Host '  envbyte register     create an account'
    Write-Host '  envbyte create app   start a project in this directory'
    Write-Host 'Docs: https://envbyte.trackedge.in'
}
