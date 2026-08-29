Add-Type -AssemblyName System.Drawing

$OutputPath = Join-Path $PSScriptRoot "unity-style-ai-editor-template.png"
$W = 1600
$H = 1000
$bmp = [System.Drawing.Bitmap]::new($W, $H)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::ClearTypeGridFit

function C($hex) { [System.Drawing.ColorTranslator]::FromHtml($hex) }
function B($hex) { [System.Drawing.SolidBrush]::new((C $hex)) }
function P($hex, $w = 1) { [System.Drawing.Pen]::new((C $hex), $w) }
function F($size, $style = [System.Drawing.FontStyle]::Regular) {
  [System.Drawing.Font]::new("Segoe UI", [single]$size, $style, [System.Drawing.GraphicsUnit]::Pixel)
}
function DrawRect($x, $y, $w, $h, $fill, $stroke = $null) {
  $g.FillRectangle((B $fill), $x, $y, $w, $h)
  if ($stroke) { $g.DrawRectangle((P $stroke), $x, $y, $w, $h) }
}
function T($text, $x, $y, $size = 12, $color = "#D7DCE2", $style = [System.Drawing.FontStyle]::Regular) {
  $font = F $size $style
  $g.DrawString($text, $font, (B $color), [single]$x, [single]$y)
  $font.Dispose()
}
function CT($text, $x, $y, $w, $h, $size = 12, $color = "#D7DCE2", $style = [System.Drawing.FontStyle]::Regular) {
  $font = F $size $style
  $fmt = [System.Drawing.StringFormat]::new()
  $fmt.Alignment = [System.Drawing.StringAlignment]::Center
  $fmt.LineAlignment = [System.Drawing.StringAlignment]::Center
  $rect = [System.Drawing.RectangleF]::new([single]$x, [single]$y, [single]$w, [single]$h)
  $g.DrawString($text, $font, (B $color), $rect, $fmt)
  $fmt.Dispose()
  $font.Dispose()
}
function Tab($text, $x, $y, $w, $active = $false) {
  DrawRect $x $y $w 30 ($(if ($active) { "#2E333B" } else { "#24282E" })) "#1B1E23"
  CT $text $x $y $w 30 12 ($(if ($active) { "#FFFFFF" } else { "#A9B1BC" })) ($(if ($active) { [System.Drawing.FontStyle]::Bold } else { [System.Drawing.FontStyle]::Regular }))
}
function Button($text, $x, $y, $w, $h = 28, $fill = "#343A43", $color = "#DDE4EC") {
  DrawRect $x $y $w $h $fill "#48505A"
  CT $text $x $y $w $h 12 $color
}
function TreeItem($text, $x, $y, $w, $active = $false, $indent = 0, $color = "#9BA5B2") {
  if ($active) { DrawRect $x $y $w 26 "#314A68" "#5C7EA7" }
  T $text ($x + 10 + $indent) ($y + 6) 12 ($(if ($active) { "#FFFFFF" } else { $color }))
}
function Field($label, $value, $x, $y, $w) {
  T $label $x ($y + 5) 11 "#9BA5B2"
  DrawRect ($x + 98) $y ($w - 98) 24 "#1E2228" "#3E4650"
  T $value ($x + 106) ($y + 5) 11 "#DDE4EC"
}
function Component($title, $x, $y, $w, $rows) {
  $h = 34 + $rows.Count * 30
  DrawRect $x $y $w $h "#282D34" "#3E4650"
  DrawRect $x $y $w 34 "#303640" "#3E4650"
  T $title ($x + 10) ($y + 9) 12 "#FFFFFF" ([System.Drawing.FontStyle]::Bold)
  Button "AI" ($x + $w - 42) ($y + 6) 30 22 "#23413C" "#BFF7EA"
  $cy = $y + 42
  foreach ($row in $rows) {
    Field $row[0] $row[1] ($x + 10) $cy ($w - 20)
    $cy += 30
  }
}
function Asset($name, $x, $y, $accent) {
  DrawRect $x $y 112 96 "#262B32" "#3E4650"
  DrawRect ($x + 10) ($y + 10) 92 56 "#1B1F25" "#343C46"
  $g.FillEllipse((B $accent), $x + 45, $y + 27, 32, 20)
  T $name ($x + 10) ($y + 72) 10 "#BEC7D2"
}

DrawRect 0 0 $W $H "#1F2329"

# Top menu and toolbar, Unity-like.
DrawRect 0 0 $W 30 "#20242A" "#15181D"
T "File   Edit   Assets   GameObject   Component   AI   Window   Help" 16 8 12 "#D7DCE2"
DrawRect 0 30 $W 44 "#2A2F36" "#1A1D22"
Button "Move" 16 38 58 28
Button "Rotate" 82 38 66 28
Button "Scale" 156 38 58 28
Button "Ask AI" 224 38 72 28 "#23413C" "#BFF7EA"
Button "Play" 726 37 52 30 "#1F5D48" "#DFFFEF"
Button "Pause" 784 37 58 30
Button "Step" 848 37 52 30
Button "Cloud AI: Project Context On" 1320 38 210 28 "#2E415D" "#DCEBFF"

$top = 74
$bottom = 760
$leftW = 250
$rightW = 350
$centerW = $W - $leftW - $rightW

# Left hierarchy.
DrawRect 0 $top $leftW ($bottom - $top) "#252A31" "#15181D"
Tab "Hierarchy" 0 $top 126 $true
Tab "Systems" 126 $top 124 $false
TreeItem "SampleScene" 10 118 230 $true 0
TreeItem "Main Camera" 10 150 230 $false 18
TreeItem "Directional Light" 10 180 230 $false 18
TreeItem "Player" 10 210 230 $false 18
TreeItem "EnemySpawner" 10 240 230 $false 18
TreeItem "Enemy_Medium_A" 10 270 230 $true 18
TreeItem "Canvas" 10 300 230 $false 18
TreeItem "GameManager" 10 330 230 $false 18
DrawRect 12 650 226 84 "#20252B" "#3E4650"
T "AI Context" 24 664 12 "#BFF7EA" ([System.Drawing.FontStyle]::Bold)
T "Selected: Enemy_Medium_A" 24 688 11 "#BEC7D2"
T "References: 2 prefabs, 1 scene" 24 710 11 "#BEC7D2"

# Center scene/game.
$cx = $leftW
DrawRect $cx $top $centerW ($bottom - $top) "#171A1F" "#15181D"
Tab "Scene" $cx $top 90 $true
Tab "Game" ($cx + 90) $top 90 $false
Tab "Asset Preview" ($cx + 180) $top 120 $false
Button "2D" ($cx + $centerW - 260) ($top + 4) 44 24
Button "Gizmos" ($cx + $centerW - 210) ($top + 4) 64 24
Button "AI Overlay" ($cx + $centerW - 138) ($top + 4) 112 24 "#23413C" "#BFF7EA"

DrawRect ($cx + 1) ($top + 31) ($centerW - 2) ($bottom - $top - 32) "#11151A"
for ($i = $cx + 1; $i -lt $cx + $centerW; $i += 32) { $g.DrawLine((P "#202833"), $i, $top + 32, $i, $bottom) }
for ($j = $top + 32; $j -lt $bottom; $j += 32) { $g.DrawLine((P "#202833"), $cx + 1, $j, $cx + $centerW, $j) }
DrawRect ($cx + 160) ($top + 130) ($centerW - 320) 420 "#1D232B" "#48617B"
Button "AI Preview: Candidate B applied temporarily" ($cx + 190) ($top + 156) 270 28 "#202A31"
Button "Before / After" ($cx + 470) ($top + 156) 116 28 "#202A31"
Button "6 / 7 Checks" ($cx + 594) ($top + 156) 100 28 "#202A31"

$sx = $cx + [int]($centerW / 2)
$sy = $top + 370
$ship = [System.Drawing.Drawing2D.GraphicsPath]::new()
$ship.AddPolygon(@(
  [System.Drawing.PointF]::new($sx, $sy - 66),
  [System.Drawing.PointF]::new($sx + 30, $sy - 18),
  [System.Drawing.PointF]::new($sx + 78, $sy),
  [System.Drawing.PointF]::new($sx + 30, $sy + 20),
  [System.Drawing.PointF]::new($sx + 18, $sy + 70),
  [System.Drawing.PointF]::new($sx, $sy + 36),
  [System.Drawing.PointF]::new($sx - 18, $sy + 70),
  [System.Drawing.PointF]::new($sx - 30, $sy + 20),
  [System.Drawing.PointF]::new($sx - 78, $sy),
  [System.Drawing.PointF]::new($sx - 30, $sy - 18)
))
$g.FillPath((B "#C83A4C"), $ship)
$g.DrawPath((P "#FF8D95" 2), $ship)
$ship.Dispose()
T "Scene View keeps Unity operation style. AI only adds preview overlays and contextual actions." ($cx + 205) ($bottom - 52) 12 "#8793A3"

# Right inspector with AI dock, not a full extra workspace.
$rx = $leftW + $centerW
DrawRect $rx $top $rightW ($bottom - $top) "#252A31" "#15181D"
Tab "Inspector" $rx $top 116 $true
Tab "AI Assistant" ($rx + 116) $top 118 $false
Tab "Asset" ($rx + 234) $top 116 $false
Component "Transform" ($rx + 12) ($top + 44) ($rightW - 24) @(
  @("Position", "0, 1.2, 0"),
  @("Rotation", "0, 0, 0"),
  @("Scale", "1, 1, 1")
)
Component "Sprite Renderer" ($rx + 12) ($top + 178) ($rightW - 24) @(
  @("Sprite", "enemy_medium_B"),
  @("Material", "red_faction_mat"),
  @("Sorting", "Enemy")
)
Component "Enemy Behavior" ($rx + 12) ($top + 312) ($rightW - 24) @(
  @("DSL", "enemy_zigzag.dsl"),
  @("Speed", "3.8"),
  @("Health", "45")
)
DrawRect ($rx + 12) ($top + 474) ($rightW - 24) 176 "#1E302E" "#3F756C"
T "AI Assistant Dock" ($rx + 24) ($top + 488) 13 "#BFF7EA" ([System.Drawing.FontStyle]::Bold)
T "Selection-aware, dockable like a Unity window." ($rx + 24) ($top + 512) 11 "#BEC7D2"
DrawRect ($rx + 24) ($top + 540) ($rightW - 48) 42 "#17201F" "#3F756C"
T "Make this enemy more threatening." ($rx + 34) ($top + 552) 12 "#FFFFFF"
Button "Generate Variants" ($rx + 24) ($top + 596) 136 28 "#23413C" "#BFF7EA"
Button "Create Patch Plan" ($rx + 170) ($top + 596) 136 28 "#343A43"

# Bottom project, console and AI tasks.
DrawRect 0 $bottom $W ($H - $bottom) "#20242A" "#15181D"
Tab "Project" 0 $bottom 96 $true
Tab "Console" 96 $bottom 96 $false
Tab "AI Tasks" 192 $bottom 96 $false
Tab "Build Report" 288 $bottom 112 $false

DrawRect 0 ($bottom + 31) 250 ($H - $bottom - 31) "#252A31" "#15181D"
TreeItem "Assets" 10 ($bottom + 52) 230 $true 0
TreeItem "Scenes" 10 ($bottom + 84) 230 $false 18
TreeItem "Prefabs" 10 ($bottom + 114) 230 $false 18
TreeItem "Sprites" 10 ($bottom + 144) 230 $false 18
TreeItem "AI Generated" 10 ($bottom + 174) 230 $false 18

DrawRect 250 ($bottom + 31) 780 ($H - $bottom - 31) "#191D22" "#15181D"
Asset "enemy_small" 274 816 "#4FA3FF"
Asset "enemy_medium_B" 410 816 "#D94B5A"
Asset "enemy_boss" 546 816 "#F6C156"
Asset "impact_vfx" 682 816 "#9A7CFF"
Asset "red_mat" 818 816 "#34C7A8"

DrawRect 1030 ($bottom + 31) 570 ($H - $bottom - 31) "#171B20" "#15181D"
T "AI Tasks / Plan Preview" 1050 804 13 "#FFFFFF" ([System.Drawing.FontStyle]::Bold)
T "1. Generate 3 sprite variants from selected Enemy_Medium_A" 1050 838 12 "#BEC7D2"
T "2. Preserve pivot, collision, prefab reference, mobile texture budget" 1050 866 12 "#BEC7D2"
T "3. Validate Style Bible and AssetSlot before applying" 1050 894 12 "#BEC7D2"
Button "Review Diff" 1050 930 120 30
Button "Apply to Project" 1184 930 136 30 "#23413C" "#BFF7EA"
Button "Cancel" 1334 930 88 30

$bmp.Save($OutputPath, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose()
$bmp.Dispose()
Write-Output $OutputPath

