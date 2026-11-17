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
    $ProgressPreference = 'SilentlyContinue' # the progress bar makes Invoke-WebRequest very slow on 5.1
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

    $repo = 'https://github.com/theallmyti/Vitals/releases/latest/download'

    if (-not [Environment]::Is64BitOperatingSystem) {
        Write-Host 'Vitals needs 64-bit Windows 10 or 11.' -ForegroundColor Red
        return
    }

    $tmp = Join-Path ([IO.Path]::GetTempPath()) ("vitals-" + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        Write-Host 'Checking the latest Vitals release...'
        $sums = Join-Path $tmp 'SHA256SUMS'
        Invoke-WebRequest -UseBasicParsing -Uri "$repo/SHA256SUMS" -OutFile $sums

        # SHA256SUMS is also the list of files: it names the setup and its checksum.
        $line = Get-Content $sums | Where-Object { $_ -match '^[0-9a-f]{64}  vitals-setup-[0-9.]+\.exe$' } | Select-Object -First 1
        if (-not $line) {
            Write-Host 'The latest release has no Windows setup.' -ForegroundColor Red
            return
        }
        $want, $file = $line -split '  ', 2

        Write-Host "Downloading $file..."
        $exe = Join-Path $tmp $file
        Invoke-WebRequest -UseBasicParsing -Uri "$repo/$file" -OutFile $exe

        $got = (Get-FileHash -Algorithm SHA256 -Path $exe).Hash.ToLower()
        if ($got -ne $want) {
            Write-Host "Checksum mismatch for $file; nothing was installed. Try again." -ForegroundColor Red
            return
        }
        Write-Host 'Checksum OK. Starting the setup (Windows will ask for admin rights)...'

        $setup = Start-Process -FilePath $exe -Wait -PassThru
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
