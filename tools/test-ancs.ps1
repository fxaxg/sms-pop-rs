param(
    [switch]$CleanOnly,
    [ValidateRange(0, 30)]
    [int]$SettleSeconds = 5
)

$ErrorActionPreference = 'Stop'
$ProjectRoot = Split-Path -Parent $PSScriptRoot
function Get-SmsPopTestProcesses {
    $rootPattern = [regex]::Escape($ProjectRoot)

    Get-CimInstance Win32_Process | Where-Object {
        $name = $_.Name
        $path = [string]$_.ExecutablePath
        $commandLine = [string]$_.CommandLine

        # SmsPop/ANCS 程序会独占或扰动同一条 BLE 链路，测试前必须全部退出。
        $isBluetoothTestApp =
            $name -in @('smspop-app.exe', 'AncsMvp.exe') -or
            ($name -eq 'SmsPop.exe' -and $path -match '[\\/]SmsPop[\\/]')

        # 只清理由本仓库启动的 Node/Vite/Tauri 开发进程，不误杀其他项目的 node.exe。
        $isProjectDevProcess =
            $name -in @('node.exe', 'npm.exe', 'npm.cmd') -and
            $commandLine -match $rootPattern -and
            $commandLine -match '(vite|tauri)'

        $isBluetoothTestApp -or $isProjectDevProcess
    }
}

function Get-ProcessTree {
    param([object[]]$Roots)

    $all = @(Get-CimInstance Win32_Process)
    $ids = [System.Collections.Generic.HashSet[int]]::new()
    foreach ($root in $Roots) { [void]$ids.Add([int]$root.ProcessId) }

    do {
        $added = $false
        foreach ($process in $all) {
            if ($ids.Contains([int]$process.ParentProcessId) -and $ids.Add([int]$process.ProcessId)) {
                $added = $true
            }
        }
    } while ($added)

    # Stop children before their parents.
    @($all | Where-Object { $ids.Contains([int]$_.ProcessId) } | Sort-Object ProcessId -Descending)
}

function Stop-SmsPopTestProcesses {
    param([string]$Reason)

    $roots = @(Get-SmsPopTestProcesses)
    if ($roots.Count -eq 0) {
        Write-Host '[clean] No stale SmsPop/ANCS test processes found.' -ForegroundColor DarkGray
        return
    }

    $processes = @(Get-ProcessTree -Roots $roots)

    Write-Host "[clean] $Reason" -ForegroundColor Yellow
    foreach ($process in $processes) {
        $displayPath = if ($process.ExecutablePath) { $process.ExecutablePath } else { $process.CommandLine }
        Write-Host "        PID=$($process.ProcessId) $($process.Name) $displayPath"
        Stop-Process -Id $process.ProcessId -Force -ErrorAction SilentlyContinue
    }

    $deadline = (Get-Date).AddSeconds(10)
    do {
        Start-Sleep -Milliseconds 250
        $remaining = @(Get-SmsPopTestProcesses)
    } while ($remaining.Count -gt 0 -and (Get-Date) -lt $deadline)

    if ($remaining.Count -gt 0) {
        $ids = ($remaining.ProcessId -join ', ')
        throw "Test processes did not exit: $ids. Close them or restart Windows before testing."
    }
}

function Wait-BluetoothRelease {
    if ($SettleSeconds -le 0) { return }

    Write-Host "[wait] Waiting $SettleSeconds seconds for the Windows Bluetooth stack..." -ForegroundColor Cyan
    for ($remaining = $SettleSeconds; $remaining -gt 0; $remaining--) {
        Write-Progress -Activity 'Waiting for Bluetooth resources' -Status "$remaining seconds" `
            -PercentComplete ((($SettleSeconds - $remaining) / $SettleSeconds) * 100)
        Start-Sleep -Seconds 1
    }
    Write-Progress -Activity 'Waiting for Bluetooth resources' -Completed
}

if ($env:OS -ne 'Windows_NT') {
    throw 'This script only runs on Windows.'
}

Push-Location $ProjectRoot
try {
    Write-Host 'SmsPop clean ANCS test' -ForegroundColor Green
    Write-Host "Project: $ProjectRoot"

    Stop-SmsPopTestProcesses -Reason 'Stopping processes that may hold Bluetooth resources...'
    Wait-BluetoothRelease

    if ($CleanOnly) {
        Write-Host '[done] Cleanup complete. SmsPop was not started.' -ForegroundColor Green
        return
    }

    if (-not (Get-Command npm.cmd -ErrorAction SilentlyContinue)) {
        throw 'npm.cmd was not found. Install Node.js and add npm to PATH.'
    }

    Write-Host '[start] Starting npm run tauri dev. Press Ctrl+C after the test.' -ForegroundColor Green
    Write-Host '[note] Closing the settings window only hides it to the tray. This script cleans up again on exit.' -ForegroundColor DarkYellow

    # 直接运行，保留完整实时日志和 Ctrl+C 行为。
    & npm.cmd run tauri dev
    if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne 130) {
        Write-Warning "tauri dev exit code: $LASTEXITCODE"
    }
}
finally {
    Write-Host "`n[exit] Cleaning up this test run..." -ForegroundColor Yellow
    Stop-SmsPopTestProcesses -Reason 'Releasing SmsPop and development-server resources.'
    Pop-Location
}
