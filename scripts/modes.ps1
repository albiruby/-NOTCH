param(
    [string]$Exe = "D:\NOTCH - Rust Project\target\release\notch.exe",
    [string]$Out = "$env:TEMP\notch_modes"
)

Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class ModeProbe {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc p, IntPtr l);
    delegate bool EnumProc(IntPtr h, IntPtr l);
    public static IntPtr FindByClass(string cls) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => {
            var sb = new StringBuilder(256);
            GetClassName(h, sb, 256);
            if (sb.ToString() == cls) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
"@
[void][ModeProbe]::SetProcessDPIAware()
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

$appdata = Join-Path $env:TEMP "notch-mode-appdata"
Remove-Item $appdata -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$appdata\Notch" | Out-Null
'{"mode":"island"}' | Set-Content "$appdata\Notch\settings.json" -Encoding utf8
$env:APPDATA = $appdata

$proc = Start-Process -FilePath $Exe -ArgumentList @("--expanded") -PassThru
Start-Sleep -Milliseconds 1500
Capture "$Out\1-island-expanded.png"

$trayHost = [ModeProbe]::FindByClass("NotchTrayHost")
"trayHost=$trayHost"
if ($trayHost -ne [IntPtr]::Zero) {
    [void][ModeProbe]::PostMessage($trayHost, 0x0111, [IntPtr]4, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 400
    Capture "$Out\2-notch-expanded.png"
    [void][ModeProbe]::PostMessage($trayHost, 0x0111, [IntPtr]5, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 400
}

Stop-Process -Id $proc.Id -Force
Start-Sleep -Milliseconds 300

$proc = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Milliseconds 1300
Capture "$Out\3-island-compact.png"
$trayHost = [ModeProbe]::FindByClass("NotchTrayHost")
if ($trayHost -ne [IntPtr]::Zero) {
    [void][ModeProbe]::PostMessage($trayHost, 0x0111, [IntPtr]4, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 400
    Capture "$Out\4-notch-compact.png"
}
Stop-Process -Id $proc.Id -Force
"saved to $Out"
