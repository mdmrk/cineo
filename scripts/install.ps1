$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$Repo = 'mdmrk/cineo'
$Version = if ($env:CINEO_VERSION) { $env:CINEO_VERSION } else { 'latest' }
$InstallDir = Join-Path $env:LOCALAPPDATA 'Programs\Cineo'
$Exe = Join-Path $InstallDir 'cineo-desktop.exe'
$Shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'Cineo.lnk'
$Extracted = Join-Path $env:LOCALAPPDATA 'Cineo\cache\libmpv'

if ($env:CINEO_UNINSTALL) {
    Remove-Item -Force -ErrorAction SilentlyContinue $Shortcut
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $InstallDir, $Extracted
    Write-Host "Cineo removed. Your addons and library stay in $env:APPDATA\Cineo."
    return
}

if (-not [Environment]::Is64BitOperatingSystem -or $env:PROCESSOR_ARCHITECTURE -eq 'ARM64') {
    throw 'There is only an x86_64 build of Cineo for Windows yet.'
}

$Base = if ($Version -eq 'latest') {
    "https://github.com/$Repo/releases/latest/download"
} else {
    "https://github.com/$Repo/releases/download/$Version"
}

$Tmp = Join-Path ([IO.Path]::GetTempPath()) ("cineo-" + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $Tmp | Out-Null
try {
    Write-Host "Downloading Cineo ($Version)..."
    $Files = 'cineo-x86_64-windows.exe', 'cineo-licenses.zip'
    foreach ($File in $Files + 'SHA256SUMS') {
        Invoke-WebRequest -UseBasicParsing -Uri "$Base/$File" -OutFile (Join-Path $Tmp $File)
    }
    $Sums = @{}
    foreach ($Line in Get-Content (Join-Path $Tmp 'SHA256SUMS')) {
        $Hash, $Name = $Line -split '\s+\*?', 2
        $Sums[$Name] = $Hash
    }
    foreach ($File in $Files) {
        $Actual = (Get-FileHash -Algorithm SHA256 (Join-Path $Tmp $File)).Hash
        if (-not $Sums[$File] -or $Actual -ne $Sums[$File].ToUpperInvariant()) {
            throw "Checksum mismatch for $File; nothing was installed."
        }
    }

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    try {
        Move-Item -Force (Join-Path $Tmp 'cineo-x86_64-windows.exe') $Exe
    } catch {
        throw 'Could not replace cineo-desktop.exe. Close Cineo and run the installer again.'
    }
    $Licenses = Join-Path $InstallDir 'licenses'
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Licenses
    Expand-Archive -Force (Join-Path $Tmp 'cineo-licenses.zip') $InstallDir
    Rename-Item (Join-Path $InstallDir 'cineo-licenses') 'licenses'

    $Shell = New-Object -ComObject WScript.Shell
    $Link = $Shell.CreateShortcut($Shortcut)
    $Link.TargetPath = $Exe
    $Link.WorkingDirectory = $InstallDir
    $Link.Description = 'Cineo media center'
    $Link.Save()
} finally {
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Tmp
}

Write-Host "Installed $Exe"
Write-Host 'Start Cineo from the Start menu.'
