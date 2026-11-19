# Builds assets/notch.ico and assets/logo-mark.png from NOTCHLogoSingle.png.
# Run from the repo root: powershell -ExecutionPolicy Bypass -File scripts\make-icons.ps1
Add-Type -AssemblyName System.Drawing
$root = Split-Path $PSScriptRoot -Parent
$src = [System.Drawing.Image]::FromFile((Join-Path $root "NOTCHLogoSingle.png"))
$assets = Join-Path $root "assets"
New-Item -ItemType Directory -Force $assets | Out-Null

# The mark sits around (830, 497) in the source; take a square around it,
# padded with black where it runs past the image.
$cx = 830; $cy = 497; $side = 1300

function Render([int]$size, [bool]$rounded) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'; $g.InterpolationMode = 'HighQualityBicubic'; $g.PixelOffsetMode = 'HighQuality'
    $g.Clear([System.Drawing.Color]::Transparent)
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    if ($rounded) {
        $r = [Math]::Max(2, [int]($size * 0.22)); $d = $r * 2; $e = $size - 1
        $path.AddArc(0, 0, $d, $d, 180, 90); $path.AddArc($e - $d, 0, $d, $d, 270, 90)
        $path.AddArc($e - $d, $e - $d, $d, $d, 0, 90); $path.AddArc(0, $e - $d, $d, $d, 90, 90)
        $path.CloseFigure()
    } else { $path.AddRectangle((New-Object System.Drawing.Rectangle 0, 0, $size, $size)) }
    $g.SetClip($path)
    $g.Clear([System.Drawing.Color]::FromArgb(255, 8, 6, 18))
    $srcRect = New-Object System.Drawing.RectangleF ($cx - $side / 2), ($cy - $side / 2), $side, $side
    $dstRect = New-Object System.Drawing.RectangleF 0, 0, $size, $size
    $g.DrawImage($src, $dstRect, $srcRect, [System.Drawing.GraphicsUnit]::Pixel)
    $g.Dispose()
    return $bmp
}

function PngBytes($bmp) {
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    return ,$ms.ToArray()
}

# Square mark for the renderer (clipped to a disc at draw time).
$mark = Render 256 $false
$mark.Save((Join-Path $assets "logo-mark.png"), [System.Drawing.Imaging.ImageFormat]::Png)

# Multi-size ICO with PNG-compressed entries.
$sizes = 16, 20, 24, 32, 40, 48, 64, 128, 256
$images = foreach ($s in $sizes) { ,(PngBytes (Render $s $true)) }
$out = New-Object System.IO.MemoryStream
$w = New-Object System.IO.BinaryWriter $out
$w.Write([UInt16]0); $w.Write([UInt16]1); $w.Write([UInt16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $s = $sizes[$i]; $b = if ($s -ge 256) { 0 } else { $s }
    $w.Write([byte]$b); $w.Write([byte]$b); $w.Write([byte]0); $w.Write([byte]0)
    $w.Write([UInt16]1); $w.Write([UInt16]32)
    $w.Write([UInt32]$images[$i].Length); $w.Write([UInt32]$offset)
    $offset += $images[$i].Length
}
foreach ($img in $images) { $w.Write($img) }
[System.IO.File]::WriteAllBytes((Join-Path $assets "notch.ico"), $out.ToArray())
$src.Dispose()
"icons written to $assets"
