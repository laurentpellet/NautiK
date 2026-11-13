<#
.SYNOPSIS
    Mesure la consommation CPU et mémoire de nautikd sous charge AIS (Windows).

.DESCRIPTION
    Compile nautikd et le multiplexeur simulé (exemple nmea_feed) en mode
    release, lance les deux sur la boucle locale avec un dossier de données
    temporaire, branche nautikd sur le flux NMEA via l'API, puis relève le
    processus nautikd toutes les 2 secondes. Le dossier temporaire est
    supprimé à la fin. Aucune donnée ne quitte la machine.

.EXAMPLE
    ./scripts/measure-nautikd.ps1 -Targets 1000 -Seconds 120
#>
param(
    [ValidateRange(0, 20000)] [int] $Targets = 1000,
    [ValidateRange(10, 3600)] [int] $Seconds = 120,
    [int] $ApiPort = 8391,
    [int] $FeedPort = 10110
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    # Cargo écrit sa progression sur stderr : ce n'est pas une erreur.
    $ErrorActionPreference = 'Continue'
    cargo build --release --locked -p nautik-daemon --bin nautikd -p nav-engine --example nmea_feed 2>&1 |
        ForEach-Object { "$_" }
    $built = $LASTEXITCODE -eq 0
    $ErrorActionPreference = 'Stop'
    if (-not $built) { throw 'compilation impossible' }
} finally {
    Pop-Location
}

$daemonExe = Join-Path $root 'target\release\nautikd.exe'
$feedExe = Join-Path $root 'target\release\examples\nmea_feed.exe'
$data = Join-Path ([System.IO.Path]::GetTempPath()) ("nautik-mesure-" + [guid]::NewGuid().ToString('N'))
$base = "http://127.0.0.1:$ApiPort"
$feed = $null
$daemon = $null

try {
    $feed = Start-Process -FilePath $feedExe -ArgumentList '--port', $FeedPort, '--targets', $Targets `
        -PassThru -WindowStyle Hidden
    $env:RUST_LOG = 'warn'
    $daemon = Start-Process -FilePath $daemonExe -ArgumentList '--data-dir', "`"$data`"", '--listen', "127.0.0.1:$ApiPort" `
        -PassThru -WindowStyle Hidden
    $started = Get-Date

    $ready = $false
    for ($i = 0; $i -lt 100 -and -not $ready; $i++) {
        try {
            Invoke-RestMethod "$base/health" -TimeoutSec 2 | Out-Null
            $ready = $true
        } catch {
            Start-Sleep -Milliseconds 200
        }
    }
    if (-not $ready) { throw 'nautikd ne répond pas' }
    $startup = ((Get-Date) - $started).TotalMilliseconds

    $token = (Get-Content (Join-Path $data 'api-token') -Raw).Trim()
    $headers = @{ Authorization = "Bearer $token" }
    $body = @{ mode = 'nmea0183'; nmea = @{ kind = 'tcp'; host = '127.0.0.1'; port = $FeedPort } } |
        ConvertTo-Json -Compress
    Invoke-RestMethod "$base/api/v1/sources" -Method Put -Headers $headers `
        -ContentType 'application/json' -Body $body | Out-Null

    Write-Host "Mesure : $Targets cibles AIS pendant $Seconds s (échauffement 10 s)..."
    Start-Sleep -Seconds 10
    $cores = [Environment]::ProcessorCount
    $samples = @()
    $proc = Get-Process -Id $daemon.Id
    $lastCpu = $proc.TotalProcessorTime.TotalSeconds
    $lastAt = Get-Date
    $end = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $end) {
        Start-Sleep -Seconds 2
        $proc.Refresh()
        $now = Get-Date
        $cpu = $proc.TotalProcessorTime.TotalSeconds
        $samples += [pscustomobject]@{
            # Pourcentage d'un cœur.
            Cpu = 100 * ($cpu - $lastCpu) / ($now - $lastAt).TotalSeconds
            RamMiB = $proc.WorkingSet64 / 1MB
            PrivateMiB = $proc.PrivateMemorySize64 / 1MB
        }
        $lastCpu = $cpu
        $lastAt = $now
    }

    # JSON en UTF-8 : décodé explicitement (Windows PowerShell 5.1 suppose
    # Latin-1 sans « charset »).
    $raw = Invoke-WebRequest "$base/api/v1/state" -Headers $headers -UseBasicParsing
    $state = [Text.Encoding]::UTF8.GetString($raw.RawContentStream.ToArray()) | ConvertFrom-Json
    $ais = ($state | Where-Object { $_.type -eq 'ais' }).data.targets.Count
    $rejected = ($state | Where-Object { $_.type -eq 'rejections' }).data
    $rejections = "{0} (dernière cause : {1})" -f $rejected.count, $rejected.lastReason
    $cpuStats = $samples | Measure-Object -Property Cpu -Average -Maximum
    $ramStats = $samples | Measure-Object -Property RamMiB -Minimum -Maximum
    $private = ($samples | Select-Object -Last 1).PrivateMiB

    Write-Host ''
    Write-Host ("Machine          : {0} cœurs, {1}" -f $cores, [Environment]::OSVersion.VersionString)
    Write-Host ("Cibles suivies   : {0} / {1}" -f $ais, $Targets)
    Write-Host ("Rejets du flux   : {0}" -f $rejections)
    Write-Host ("Démarrage        : {0:N0} ms" -f $startup)
    Write-Host ("CPU (1 cœur)     : moyenne {0:N1} %, pic {1:N1} %" -f $cpuStats.Average, $cpuStats.Maximum)
    Write-Host ("RAM (working set): {0:N1} à {1:N1} Mio" -f $ramStats.Minimum, $ramStats.Maximum)
    Write-Host ("RAM privée (fin) : {0:N1} Mio" -f $private)
} finally {
    Remove-Item Env:RUST_LOG -ErrorAction SilentlyContinue
    foreach ($p in @($daemon, $feed)) {
        if ($p -and -not $p.HasExited) { Stop-Process -Id $p.Id -Force }
    }
    Start-Sleep -Milliseconds 500
    if (Test-Path $data) { Remove-Item -Recurse -Force $data }
}
