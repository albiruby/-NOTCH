param(
    [string]$Exe = "D:\NOTCH - Rust Project\target\release\notch.exe",
    [string]$Out = "$env:TEMP\notch_media"
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class MediaProbe {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string name);
}
"@
[void][MediaProbe]::SetProcessDPIAware()
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
New-Item -ItemType Directory -Force -Path $Out | Out-Null

$wav = Join-Path $env:TEMP "notch-test.wav"
$rate = 44100
$seconds = 40
$samples = $rate * $seconds
$data = New-Object byte[] ($samples * 2)
for ($i = 0; $i -lt $samples; $i++) {
    $v = [int16](8000 * [math]::Sin(2 * [math]::PI * 220 * $i / $rate))
    $data[2 * $i] = $v -band 0xFF
    $data[2 * $i + 1] = ($v -shr 8) -band 0xFF
}
$stream = [System.IO.File]::Create($wav)
$writer = New-Object System.IO.BinaryWriter($stream)
$writer.Write([Text.Encoding]::ASCII.GetBytes("RIFF"))
$writer.Write([int32](36 + $data.Length))
$writer.Write([Text.Encoding]::ASCII.GetBytes("WAVEfmt "))
$writer.Write([int32]16)
$writer.Write([int16]1)
$writer.Write([int16]1)
$writer.Write([int32]$rate)
$writer.Write([int32]($rate * 2))
$writer.Write([int16]2)
$writer.Write([int16]16)
$writer.Write([Text.Encoding]::ASCII.GetBytes("data"))
$writer.Write([int32]$data.Length)
$writer.Write($data)
$writer.Close()
$stream.Close()

Add-Type -AssemblyName System.Runtime.WindowsRuntime
[Windows.Media.Playback.MediaPlayer, Windows.Media, ContentType = WindowsRuntime] | Out-Null
[Windows.Media.Playback.MediaPlaybackType, Windows.Media, ContentType = WindowsRuntime] | Out-Null
[Windows.Media.Core.MediaSource, Windows.Media.Core, ContentType = WindowsRuntime] | Out-Null

$player = [Windows.Media.Playback.MediaPlayer]::new()
$uri = [Uri]("file:///" + ($wav -replace '\\', '/'))
$player.Source = [Windows.Media.Core.MediaSource]::CreateFromUri($uri)
$controls = $player.SystemMediaTransportControls
$controls.DisplayUpdater.MusicProperties.Title = "Glass Surface"
$controls.DisplayUpdater.MusicProperties.Artist = "NOTCH Design System"
$controls.DisplayUpdater.Update()
$player.Play()
Start-Sleep -Seconds 2

$proc = Start-Process -FilePath $Exe -ArgumentList @("--expanded") -PassThru
Start-Sleep -Milliseconds 2500

$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap(1200, 460)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.X + [int](($b.Width - 1200) / 2), $b.Y, 0, 0, $bmp.Size)
$bmp.Save("$Out\playing.png", [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()

Stop-Process -Id $proc.Id -Force
$player.Pause()
$player.Dispose()
"done"
