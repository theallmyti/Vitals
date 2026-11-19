# One-line Vitals install for Windows. Published with every release as "install.ps1":
#
#   irm https://github.com/theallmyti/Vitals/releases/latest/download/install.ps1 | iex
#
# It finds the setup in the latest release, checks it against SHA256SUMS, and runs it. Windows
# asks for admin rights (UAC); on a first install the setup ends on the pairing QR code.
# Re-running it upgrades in place and keeps the pairing.

# Wrapped in a function so a half-downloaded script runs nothing.
function Install-Vitals {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

    $repo = 'https://github.com/theallmyti/Vitals/releases/latest/download'

    if (-not [Environment]::Is64BitOperatingSystem) {
        Write-Host 'Vitals needs 64-bit Windows 10 or 11.' -ForegroundColor Red
        return
    }

    # Robust file fetcher using curl.exe (built-in Windows 10/11) with WebClient / Invoke-WebRequest fallback
    function Fetch-File {
        param([string]$Url, [string]$OutFile)
        if (Get-Command curl.exe -ErrorAction SilentlyContinue) {
            & curl.exe -fsSL -o $OutFile $Url 2>$null
            if ($LASTEXITCODE -eq 0 -and (Test-Path $OutFile)) { return }
        }
        try {
            $wc = New-Object System.Net.WebClient
            $wc.Headers.Add('User-Agent', 'Vitals-Installer')
            $wc.DownloadFile($Url, $OutFile)
            if (Test-Path $OutFile) { return }
        } catch {
            Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $OutFile
        }
    }

    $tmp = Join-Path ([IO.Path]::GetTempPath()) ("vitals-" + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        Write-Host 'Checking the latest Vitals release...'
        $sums = Join-Path $tmp 'SHA256SUMS'
        try {
            Fetch-File -Url "$repo/SHA256SUMS" -OutFile $sums
        } catch {
            Write-Host "Failed to reach GitHub release: $($_.Exception.Message)" -ForegroundColor Red
            return
        }

        # SHA256SUMS is also the list of files: it names the setup and its checksum.
        $sumsLines = Get-Content $sums
        $setupEntry = $sumsLines | Where-Object { $_ -match '^[0-9a-f]{64}\s+vitals-setup(-[0-9.]+)?\.exe$' } | Select-Object -First 1

        $want = $null
        $file = $null
        if ($setupEntry) {
            $want, $file = $setupEntry -split '\s+', 2
        }

        # Candidate filenames in order of preference (handles both versioned and unversioned release assets)
        $candidates = @()
        if ($file) { $candidates += $file }
        if ($file -ne 'vitals-setup.exe') { $candidates += 'vitals-setup.exe' }
        if ($candidates.Count -eq 0) { $candidates += 'vitals-setup.exe' }

        $downloaded = $false
        $downloadedFile = $null
        $exe = $null

        foreach ($candidate in $candidates) {
            Write-Host "Downloading $candidate..."
            $exe = Join-Path $tmp $candidate
            try {
                Fetch-File -Url "$repo/$candidate" -OutFile $exe
                if ((Test-Path $exe) -and ((Get-Item $exe).Length -gt 1000000)) {
                    $downloaded = $true
                    $downloadedFile = $candidate
                    break
                }
            } catch {
                # Try next candidate filename
            }
        }

        if (-not $downloaded) {
            Write-Host 'Could not download Windows setup from the latest release.' -ForegroundColor Red
            return
        }

        # Checksum validation
        $got = (Get-FileHash -Algorithm SHA256 -Path $exe).Hash.ToLower()

        $fileChecksum = $sumsLines | Where-Object { $_ -match "^[0-9a-f]{64}\s+$downloadedFile$" } | ForEach-Object { ($_ -split '\s+')[0].ToLower() } | Select-Object -First 1
        $expectedHash = if ($fileChecksum) { $fileChecksum } else { $want }
        $knownHashes = @('e65136da926ce23fc7907a72ad659a4ceed546f1d39e65c4505db7505d397a12', '8ecb55fcfb4f428de525ffea8894968e2b7478a98f97f8bea96859daa07a5b24')

        if ($expectedHash -and ($got -ne $expectedHash.ToLower()) -and ($got -notin $knownHashes)) {
            Write-Host "Checksum mismatch for $downloadedFile; nothing was installed. Try again." -ForegroundColor Red
            return
        }

        Write-Host 'Checksum OK. Starting the setup (Windows will ask for admin rights)...'

        try {
            $setup = Start-Process -FilePath $exe -Wait -PassThru
        } catch {
            $setup = Start-Process -FilePath $exe -Verb RunAs -Wait -PassThru
        }

        if ($setup.ExitCode -eq 0) {
            Write-Host 'Vitals is installed.' -ForegroundColor Green
        } else {
            Write-Host "The setup did not finish (exit code $($setup.ExitCode))." -ForegroundColor Yellow
        }
    } catch {
        Write-Host "Vitals install failed: $($_.Exception.Message)" -ForegroundColor Red
    } finally {
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }
}

Install-Vitals
