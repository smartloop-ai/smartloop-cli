# Smartloop CLI installer for Windows.
#
#   irm https://raw.githubusercontent.com/smartloop-ai/smartloop-cli/main/install.ps1 | iex
#
# Environment:
#   SMARTLOOP_CLI_VERSION      Version to install (default: latest release)
#   SMARTLOOP_CLI_INSTALL_DIR  Install directory (default: $CARGO_HOME\bin when a
#                              Rust toolchain is present, else $HOME\.smartloop\bin)

$ErrorActionPreference = 'Stop'

$Repo = 'smartloop-ai/smartloop-cli'

# Where the binary lands.  A Rust toolchain already has CARGO_HOME\bin on PATH,
# so installing there means `smartloop` works straight away with no PATH edit.
# Without one, fall back to our own directory -- $HOME\.smartloop is the
# studio's data directory, so the binary sits beside it, not inside the data.
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $HOME '.cargo' }
$cargoBin = Join-Path $cargoHome 'bin'

$InstallDir = if ($env:SMARTLOOP_CLI_INSTALL_DIR) {
    $env:SMARTLOOP_CLI_INSTALL_DIR
} elseif (Test-Path -Path $cargoBin -PathType Container) {
    $cargoBin
} else {
    Join-Path $HOME '.smartloop\bin'
}

function Get-LatestVersion {
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" `
        -Headers @{ 'User-Agent' = 'smartloop-cli-installer' }
    return $release.tag_name -replace '^v', ''
}

# Only x86_64 Windows binaries are published; ARM64 Windows runs them under
# emulation, so no separate target is needed.
$arch = $env:PROCESSOR_ARCHITECTURE
if ($arch -ne 'AMD64' -and $arch -ne 'ARM64') {
    throw "Unsupported architecture: $arch"
}
$target = 'x86_64-pc-windows-msvc'

$version = if ($env:SMARTLOOP_CLI_VERSION) {
    $env:SMARTLOOP_CLI_VERSION -replace '^v', ''
} else {
    Get-LatestVersion
}
if (-not $version) { throw 'Could not determine the latest release; set SMARTLOOP_CLI_VERSION' }

$name = "smartloop-$version-$target"
$archive = "$name.zip"
$baseUrl = "https://github.com/$Repo/releases/download/v$version"

Write-Host "Installing smartloop $version ($target)"

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $tmp | Out-Null

try {
    $archivePath = Join-Path $tmp $archive
    Invoke-WebRequest -Uri "$baseUrl/$archive" -OutFile $archivePath

    $sumsPath = Join-Path $tmp 'SHA256SUMS'
    Invoke-WebRequest -Uri "$baseUrl/SHA256SUMS" -OutFile $sumsPath

    $expected = (Get-Content $sumsPath |
        Where-Object { $_ -match "\s$([regex]::Escape($archive))$" } |
        ForEach-Object { ($_ -split '\s+')[0] })
    if (-not $expected) { throw "No checksum published for $archive" }

    $actual = (Get-FileHash -Path $archivePath -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $expected.ToLower()) { throw "Checksum mismatch for $archive" }

    Expand-Archive -Path $archivePath -DestinationPath $tmp -Force

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item -Path (Join-Path $tmp "$name\smartloop.exe") -Destination $InstallDir -Force

    Write-Host "Installed $(Join-Path $InstallDir 'smartloop.exe')" -ForegroundColor Green

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($userPath -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable('Path', "$InstallDir;$userPath", 'User')
        Write-Host "Added $InstallDir to your PATH. Restart your shell to pick it up."
    }
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
