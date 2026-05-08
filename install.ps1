# install.ps1 — one-line installer for transpile (Windows)
# Usage: irm https://github.com/epicsagas/llm-transpile/releases/latest/download/install.ps1 | iex
param(
    [string]$InstallDir = "$env:USERPROFILE\.local\bin"
)

$Repo = "epicsagas/llm-transpile"
$Binary = "transpile"

# ── Detect architecture ──────────────────────────────────────────────────────
$arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
if ($arch -eq "X64") {
    $target = "x86_64-pc-windows-msvc"
} else {
    Write-Error "Error: unsupported architecture $arch"
    exit 1
}

# ── Resolve latest version ───────────────────────────────────────────────────
$tag = (Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest").tag_name
if (-not $tag) {
    Write-Error "Error: could not determine latest version"
    exit 1
}
$version = $tag.TrimStart("v")

$url = "https://github.com/$Repo/releases/download/$tag/$Binary-$target.zip"

# ── Download and install ─────────────────────────────────────────────────────
Write-Host "Installing $Binary v$version for $target..."

$tmpdir = New-Item -ItemType Directory -Path (Join-Path $env:TEMP "transpile-install") -Force
$zip = Join-Path $tmpdir "$Binary.zip"

Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing
Expand-Archive -Path $zip -DestinationPath $tmpdir -Force

if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

$src = Join-Path $tmpdir "$Binary.exe"
$dst = Join-Path $InstallDir "$Binary.exe"
Copy-Item -Path $src -Destination $dst -Force

Remove-Item -Path $tmpdir -Recurse -Force

# ── Verify ───────────────────────────────────────────────────────────────────
if (Get-Command $Binary -ErrorAction SilentlyContinue) {
    Write-Host "Installed: $Binary v$version"
} else {
    Write-Host ""
    Write-Host "Add $InstallDir to your PATH:"
    Write-Host "  `$env:PATH = `"$InstallDir;`$env:PATH`""
}
