param(
    [string]$OutputRoot = $PSScriptRoot
)

Add-Type -AssemblyName System.Drawing

$textures = @(
    @{ Name = 'tab-hover'; Fill = [System.Drawing.Color]::FromArgb(255, 74, 79, 86); Accent = [System.Drawing.Color]::FromArgb(255, 105, 111, 120) },
    @{ Name = 'tab-active'; Fill = [System.Drawing.Color]::FromArgb(255, 55, 101, 143); Accent = [System.Drawing.Color]::FromArgb(255, 88, 145, 194) },
    @{ Name = 'tab-selected'; Fill = [System.Drawing.Color]::FromArgb(255, 55, 79, 102); Accent = [System.Drawing.Color]::FromArgb(255, 70, 137, 201) },
    @{ Name = 'tab-selected-hover'; Fill = [System.Drawing.Color]::FromArgb(255, 62, 96, 127); Accent = [System.Drawing.Color]::FromArgb(255, 91, 158, 218) }
)

foreach ($texture in $textures) {
    $bitmap = [System.Drawing.Bitmap]::new(12, 12, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    try {
        for ($y = 0; $y -lt 12; $y++) {
            for ($x = 0; $x -lt 12; $x++) {
                $isCorner = (($x -lt 2 -or $x -gt 9) -and ($y -lt 2 -or $y -gt 9))
                $isBorder = $x -eq 0 -or $x -eq 11 -or $y -eq 0 -or $y -eq 11
                $isAccent = $y -eq 1 -and $x -ge 2 -and $x -le 9
                $color = if ($isCorner) {
                    [System.Drawing.Color]::Transparent
                } elseif ($isBorder) {
                    [System.Drawing.Color]::FromArgb(255, 18, 18, 18)
                } elseif ($isAccent) {
                    $texture.Accent
                } else {
                    $texture.Fill
                }
                $bitmap.SetPixel($x, $y, $color)
            }
        }
        $path = Join-Path $OutputRoot ($texture.Name + '.png')
        $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $bitmap.Dispose()
    }
}
