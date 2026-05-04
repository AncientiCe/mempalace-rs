$ErrorActionPreference = "Stop"

$Repo = if ($env:MEMPALACE_REPO) { $env:MEMPALACE_REPO } else { "AncientiCe/mempalace-rs" }
$InstallDir = if ($env:MEMPALACE_INSTALL_DIR) { $env:MEMPALACE_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Programs\mempalace\bin" }
$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("mempalace-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $TempDir | Out-Null

function Get-Target {
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    switch ($arch) {
        "X64" { "x86_64-pc-windows-msvc" }
        "Arm64" { "aarch64-pc-windows-msvc" }
        default { throw "Unsupported architecture: $arch" }
    }
}

try {
    $Target = Get-Target

    if ($env:MEMPALACE_VERSION -eq "local") {
        if (-not $env:MEMPALACE_LOCAL_ARCHIVE) {
            throw "MEMPALACE_LOCAL_ARCHIVE is required when MEMPALACE_VERSION=local"
        }
        $Archive = $env:MEMPALACE_LOCAL_ARCHIVE
    } else {
        if ($env:MEMPALACE_VERSION) {
            $Tag = $env:MEMPALACE_VERSION
        } else {
            $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
            $Tag = $Release.tag_name
        }
        $Version = $Tag.TrimStart("v")
        $Asset = "mempalace-$Version-$Target.zip"
        $Archive = Join-Path $TempDir $Asset
        $Checksum = Join-Path $TempDir "mempalace-$Target.sha256"
        Invoke-WebRequest -Uri "https://github.com/$Repo/releases/download/$Tag/$Asset" -OutFile $Archive
        Invoke-WebRequest -Uri "https://github.com/$Repo/releases/download/$Tag/mempalace-$Target.sha256" -OutFile $Checksum

        $Expected = ((Get-Content $Checksum | Select-Object -First 1) -split "\s+")[0]
        $Actual = (Get-FileHash -Algorithm SHA256 $Archive).Hash.ToLowerInvariant()
        if ($Actual -ne $Expected.ToLowerInvariant()) {
            throw "Checksum mismatch for $Asset"
        }
    }

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Expand-Archive -Path $Archive -DestinationPath $TempDir -Force
    $Binary = Get-ChildItem -Path $TempDir -Recurse -Filter "mempalace.exe" | Select-Object -First 1
    if (-not $Binary) {
        throw "Archive did not contain mempalace.exe"
    }
    Copy-Item $Binary.FullName (Join-Path $InstallDir "mempalace.exe") -Force

    $PathParts = ($env:PATH -split ";") | Where-Object { $_ }
    if ($PathParts -notcontains $InstallDir) {
        Write-Host "Add MemPalace to PATH:"
        Write-Host "  setx PATH `"$InstallDir;%PATH%`""
    }

    & (Join-Path $InstallDir "mempalace.exe") install --all

    Write-Host "MemPalace installed."
    Write-Host "Next: mempalace init <project>; mempalace mine <project>"
    Write-Host "Restart Cursor, Codex, or Claude Code to load the MCP server."
} finally {
    Remove-Item -Recurse -Force $TempDir -ErrorAction SilentlyContinue
}
