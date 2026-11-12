param(
    [string]$Exe = "D:\NOTCH - Rust Project\target\release\notch.exe",
    [string]$Out = "$env:TEMP\notch_shots",
    [switch]$Expanded
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Native {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    public struct RECT { public int L, T, R, B; }
}
"@
[void][Native]::SetProcessDPIAware()
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
New-Item -ItemType Directory -Force -Path $Out | Out-Null

function Capture([string]$Path, [int]$W = 1200, [int]$H = 460) {
    $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bmp = New-Object System.Drawing.Bitmap($W, $H)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($b.X + [int](($b.Width - $W) / 2), $b.Y, 0, 0, $bmp.Size)
    $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
}

if ($Expanded) {
    $proc = Start-Process -FilePath $Exe -ArgumentList @("--expanded") -PassThru
} else {
    $proc = Start-Process -FilePath $Exe -PassThru
}
Start-Sleep -Milliseconds 1500
$proc.Refresh()
if ($proc.HasExited) { "EXITED $($proc.ExitCode)"; exit 1 }

$hwnd = $proc.MainWindowHandle
$rect = New-Object Native+RECT
[void][Native]::GetWindowRect($hwnd, [ref]$rect)

"pid=$($proc.Id) window=$($rect.L),$($rect.T) $($rect.R - $rect.L)x$($rect.B - $rect.T)"
"ram_ws=$([math]::Round($proc.WorkingSet64 / 1MB, 1))MB ram_private=$([math]::Round($proc.PrivateMemorySize64 / 1MB, 1))MB threads=$($proc.Threads.Count)"

Capture "$Out\latest.png"

$cpu1 = $proc.TotalProcessorTime.TotalMilliseconds
Start-Sleep -Seconds 4
$proc.Refresh()
$cpu2 = $proc.TotalProcessorTime.TotalMilliseconds
$cores = [Environment]::ProcessorCount
"cpu=$([math]::Round((($cpu2 - $cpu1) / 1000 / 4) / $cores * 100, 3))% cores=$cores"

Stop-Process -Id $proc.Id -Force
"stopped"
