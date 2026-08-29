Add-Type -AssemblyName System.Drawing

$OutputPath = Join-Path $PSScriptRoot "ai-editor-interaction-template.png"

$W = 1600
$H = 1000
$bmp = New-Object System.Drawing.Bitmap $W, $H
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::ClearTypeGridFit

function C($hex) {
  return [System.Drawing.ColorTranslator]::FromHtml($hex)
}

function Brush($hex) {
  return New-Object System.Drawing.SolidBrush (C $hex)
}

function PenC($hex, $width = 1) {
  return New-Object System.Drawing.Pen (C $hex), $width
}

function FontC($size, $style = [System.Drawing.FontStyle]::Regular) {
  return [System.Drawing.Font]::new("Microsoft YaHei UI", [single]$size, $style, [System.Drawing.GraphicsUnit]::Pixel)
}

function RoundPath($x, $y, $w, $h, $r) {
  $path = New-Object System.Drawing.Drawing2D.GraphicsPath
  $d = $r * 2
  $path.AddArc($x, $y, $d, $d, 180, 90)
  $path.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
  $path.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
  $path.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
  $path.CloseFigure()
  return $path
}

function FillRound($x, $y, $w, $h, $r, $fill, $stroke = $null) {
  $path = RoundPath $x $y $w $h $r
  $g.FillPath((Brush $fill), $path)
  if ($stroke) {
    $g.DrawPath((PenC $stroke 1), $path)
  }
  $path.Dispose()
}

function Rect($x, $y, $w, $h, $fill, $stroke = $null) {
  $g.FillRectangle((Brush $fill), $x, $y, $w, $h)
  if ($stroke) {
    $g.DrawRectangle((PenC $stroke 1), $x, $y, $w, $h)
  }
}

function Text($s, $x, $y, $size = 13, $color = "#E7EDF4", $style = [System.Drawing.FontStyle]::Regular) {
  $font = FontC $size $style
  $g.DrawString($s, $font, (Brush $color), [single]$x, [single]$y)
  $font.Dispose()
}

function CenterText($s, $x, $y, $w, $h, $size = 12, $color = "#E7EDF4", $style = [System.Drawing.FontStyle]::Regular) {
  $font = FontC $size $style
  $fmt = New-Object System.Drawing.StringFormat
  $fmt.Alignment = [System.Drawing.StringAlignment]::Center
  $fmt.LineAlignment = [System.Drawing.StringAlignment]::Center
  $rect = New-Object System.Drawing.RectangleF ([single]$x), ([single]$y), ([single]$w), ([single]$h)
  $g.DrawString($s, $font, (Brush $color), $rect, $fmt)
  $fmt.Dispose()
  $font.Dispose()
}

function Chip($text, $x, $y, $w, $fill = "#303946", $color = "#AAB5C4") {
  FillRound $x $y $w 28 6 $fill "#3B4552"
  CenterText $text $x $y $w 28 12 $color
}

function PanelTitle($title, $x, $y, $w) {
  Rect $x $y $w 36 "#1D2229" "#3B4552"
  Text $title ($x + 12) ($y + 9) 12 "#AAB5C4"
}

function TreeItem($text, $x, $y, $w, $active = $false, $dot = "#748194") {
  if ($active) {
    FillRound $x $y $w 28 6 "#243145" "#426489"
  }
  $g.FillEllipse((Brush $dot), $x + 8, $y + 10, 8, 8)
  Text $text ($x + 26) ($y + 6) 12 ($(if ($active) { "#E7EDF4" } else { "#AAB5C4" }))
}

function FieldGroup($title, $x, $y, $w, $rows) {
  $h = 32 + ($rows.Count * 32)
  FillRound $x $y $w $h 8 "#252B34" "#3B4552"
  Rect $x $y $w 32 "#252B34" "#3B4552"
  Text $title ($x + 10) ($y + 8) 12 "#E7EDF4" ([System.Drawing.FontStyle]::Bold)
  $cy = $y + 32
  foreach ($row in $rows) {
    $g.DrawLine((PenC "#3B4552" 1), $x, $cy, $x + $w, $cy)
    Text $row[0] ($x + 10) ($cy + 8) 11 "#7F8B9C"
    FillRound ($x + 102) ($cy + 6) ($w - 114) 22 5 "#1C222A" "#303946"
    Text $row[1] ($x + 110) ($cy + 9) 11 "#DCE5EE"
    $cy += 32
  }
}

function Candidate($x, $y, $label, $selected = $false) {
  FillRound $x $y 96 96 7 "#252B34" ($(if ($selected) { "#32C6A6" } else { "#3B4552" }))
  $path = New-Object System.Drawing.Drawing2D.GraphicsPath
  $points = @(
    [System.Drawing.PointF]::new($x + 48, $y + 14),
    [System.Drawing.PointF]::new($x + 66, $y + 42),
    [System.Drawing.PointF]::new($x + 85, $y + 48),
    [System.Drawing.PointF]::new($x + 64, $y + 59),
    [System.Drawing.PointF]::new($x + 56, $y + 82),
    [System.Drawing.PointF]::new($x + 48, $y + 64),
    [System.Drawing.PointF]::new($x + 40, $y + 82),
    [System.Drawing.PointF]::new($x + 32, $y + 59),
    [System.Drawing.PointF]::new($x + 11, $y + 48),
    [System.Drawing.PointF]::new($x + 30, $y + 42)
  )
  $path.AddPolygon($points)
  $g.FillPath((Brush "#D94B5A"), $path)
  $g.DrawPath((PenC "#FF8F93" 1), $path)
  $path.Dispose()
  Text $label ($x + 8) ($y + 73) 11 "#AAB5C4"
}

function AssetTile($x, $y, $name, $accent = "#5AA7FF") {
  FillRound $x $y 132 112 8 "#252B34" "#3B4552"
  FillRound ($x + 10) ($y + 10) 112 70 6 "#1B2027" "#303946"
  $g.FillEllipse((Brush $accent), $x + 52, $y + 28, 36, 24)
  Text $name ($x + 10) ($y + 88) 10 "#AAB5C4"
}

# Background and top bar
Rect 0 0 $W $H "#15171B"
Rect 0 0 $W 48 "#1A1D23" "#3B4552"
FillRound 16 12 24 24 6 "#32C6A6" "#5AA7FF"
Text "AI Native Game Engine" 50 14 15 "#E7EDF4" ([System.Drawing.FontStyle]::Bold)
Text "File   Edit   Create   AI   Build   Window" 280 16 12 "#AAB5C4"
Chip "Scene" 1000 10 64
Chip "Game" 1072 10 60
Chip "Run" 1140 10 64 "#213D39" "#C9FFF2"
Chip "Build PC" 1212 10 82
Chip "AI Review Mode" 1310 10 126 "#243145" "#BFE0FF"

# Main layout dimensions
$leftX = 0; $leftW = 250
$workX = 250; $workW = 630
$inspX = 880; $inspW = 330
$aiX = 1210; $aiW = 390
$mainY = 48; $mainH = 682
$bottomY = 730; $bottomH = 270

# Left hierarchy
Rect $leftX $mainY $leftW $mainH "#20242B" "#3B4552"
PanelTitle "Hierarchy / Systems" $leftX $mainY $leftW
$ty = 96
TreeItem "Air Combat Scene" 12 $ty 226 $true "#5AA7FF"; $ty += 34
TreeItem "Player Fighter" 12 $ty 226 $false "#32C6A6"; $ty += 34
TreeItem "Enemy Squadron" 12 $ty 226 $false "#F2B84B"; $ty += 34
TreeItem "Projectile System" 12 $ty 226 $false "#A98CFF"; $ty += 34
TreeItem "Score UI" 12 $ty 226 $false "#748194"; $ty += 34
TreeItem "Explosion VFX" 12 $ty 226 $false "#748194"; $ty += 34
TreeItem "Audio Events" 12 $ty 226 $false "#748194"; $ty += 34
TreeItem "Build Profile" 12 $ty 226 $false "#748194"

# Viewport
Rect $workX $mainY $workW $mainH "#12151A" "#3B4552"
Rect $workX $mainY $workW 36 "#1D2229" "#3B4552"
Text "Scene" ($workX + 12) ($mainY + 10) 12 "#E7EDF4" ([System.Drawing.FontStyle]::Bold)
Text "Game Preview   Asset Preview   Graph" ($workX + 68) ($mainY + 10) 12 "#7F8B9C"
Chip "Mobile Quality" ($workX + 408) ($mainY + 4) 102
Chip "enemy_medium_sprite" ($workX + 518) ($mainY + 4) 104

Rect ($workX + 1) ($mainY + 37) ($workW - 2) ($mainH - 38) "#11151B"
for ($i = $workX + 1; $i -lt $workX + $workW; $i += 32) {
  $g.DrawLine((PenC "#1D252E" 1), $i, $mainY + 37, $i, $mainY + $mainH)
}
for ($j = $mainY + 37; $j -lt $mainY + $mainH; $j += 32) {
  $g.DrawLine((PenC "#1D252E" 1), $workX + 1, $j, $workX + $workW, $j)
}
FillRound ($workX + 56) ($mainY + 74) ($workW - 112) ($mainH - 132) 2 "#1C232B" "#426489"
FillRound ($workX + 75) ($mainY + 92) 190 32 6 "#121820" "#303946"
Text "Preview: selected candidate applied" ($workX + 88) ($mainY + 101) 11 "#AAB5C4"
FillRound ($workX + 274) ($mainY + 92) 145 32 6 "#121820" "#303946"
Text "Before / After" ($workX + 287) ($mainY + 101) 11 "#AAB5C4"
FillRound ($workX + 428) ($mainY + 92) 140 32 6 "#121820" "#303946"
Text "Quality 6 / 7" ($workX + 442) ($mainY + 101) 11 "#AAB5C4"

# Spaceship polygon
$sx = $workX + 315; $sy = $mainY + 348
$shipPath = New-Object System.Drawing.Drawing2D.GraphicsPath
$shipPoints = @(
  [System.Drawing.PointF]::new($sx, $sy - 58),
  [System.Drawing.PointF]::new($sx + 28, $sy - 16),
  [System.Drawing.PointF]::new($sx + 66, $sy),
  [System.Drawing.PointF]::new($sx + 28, $sy + 18),
  [System.Drawing.PointF]::new($sx + 16, $sy + 60),
  [System.Drawing.PointF]::new($sx, $sy + 30),
  [System.Drawing.PointF]::new($sx - 16, $sy + 60),
  [System.Drawing.PointF]::new($sx - 28, $sy + 18),
  [System.Drawing.PointF]::new($sx - 66, $sy),
  [System.Drawing.PointF]::new($sx - 28, $sy - 16)
)
$shipPath.AddPolygon($shipPoints)
$g.FillPath((Brush "#C73749"), $shipPath)
$g.DrawPath((PenC "#FF8F93" 2), $shipPath)
$shipPath.Dispose()
Text "Central Viewport: object preview + AI diff overlay + validation result" ($workX + 175) ($mainY + 620) 11 "#7F8B9C"

# Inspector
Rect $inspX $mainY $inspW $mainH "#20242B" "#3B4552"
PanelTitle "Inspector" $inspX $mainY $inspW
FieldGroup "Selected Entity" ($inspX + 10) ($mainY + 48) ($inspW - 20) @(
  @("Name", "Enemy_Medium_A"),
  @("Prefab", "enemy_medium.prefab"),
  @("AssetSlot", "enemy_medium_sprite")
)
FieldGroup "Components" ($inspX + 10) ($mainY + 190) ($inspW - 20) @(
  @("Transform", "position / scale"),
  @("SpriteRender", "candidate_03"),
  @("Collision", "auto from silhouette"),
  @("Behavior", "enemy_zigzag.dsl")
)
FieldGroup "Asset Governance" ($inspX + 10) ($mainY + 365) ($inspW - 20) @(
  @("Style Bible", "Red Faction Arcade"),
  @("License", "AI generated / tracked"),
  @("References", "2 prefabs, 1 scene")
)

# AI panel
Rect $aiX $mainY $aiW $mainH "#171B21" "#32C6A6"
Rect $aiX $mainY $aiW 50 "#1B252A" "#3B4552"
Text "AI Intent Workspace" ($aiX + 14) ($mainY + 9) 14 "#E7EDF4" ([System.Drawing.FontStyle]::Bold)
Text "Intent -> Spec -> Plan -> Preview -> Diff -> Validate -> Apply" ($aiX + 14) ($mainY + 29) 10 "#AAB5C4"

FillRound ($aiX + 12) ($mainY + 64) ($aiW - 24) 88 8 "#1D302E" "#2F6C64"
Text "User Intent" ($aiX + 24) ($mainY + 76) 11 "#BDF7EB" ([System.Drawing.FontStyle]::Bold)
Text "Make this medium enemy feel more threatening," ($aiX + 24) ($mainY + 100) 12 "#E7EDF4"
Text "keep red faction style, optimize for mobile." ($aiX + 24) ($mainY + 122) 12 "#E7EDF4"

$steps = @("Intent", "Spec", "Plan", "Preview", "Diff", "Validate", "Apply")
$stepX = $aiX + 12
for ($i = 0; $i -lt $steps.Count; $i++) {
  $fill = if ($i -lt 3) { "#213429" } elseif ($i -eq 3) { "#1F3A36" } else { "#252B34" }
  $color = if ($i -lt 3) { "#74D17C" } elseif ($i -eq 3) { "#C9FFF2" } else { "#7F8B9C" }
  FillRound ($stepX + $i * 52) ($mainY + 164) 48 42 6 $fill "#3B4552"
  CenterText $steps[$i] ($stepX + $i * 52) ($mainY + 164) 48 42 9 $color
}

FillRound ($aiX + 12) ($mainY + 220) ($aiW - 24) 152 8 "#252B34" "#3B4552"
Text "Asset Candidates" ($aiX + 24) ($mainY + 232) 12 "#E7EDF4" ([System.Drawing.FontStyle]::Bold)
Text "generated variants" ($aiX + 266) ($mainY + 234) 10 "#7F8B9C"
Candidate ($aiX + 24) ($mainY + 262) "A" $false
Candidate ($aiX + 146) ($mainY + 262) "B" $true
Candidate ($aiX + 268) ($mainY + 262) "C" $false

FillRound ($aiX + 12) ($mainY + 384) ($aiW - 24) 128 8 "#252B34" "#3B4552"
Text "Edit Plan" ($aiX + 24) ($mainY + 396) 12 "#E7EDF4" ([System.Drawing.FontStyle]::Bold)
Text "preserve / change" ($aiX + 266) ($mainY + 398) 10 "#7F8B9C"
Text "Preserve palette, pivot, slot" ($aiX + 24) ($mainY + 428) 11 "#AAB5C4"; Chip "locked" ($aiX + 300) ($mainY + 421) 58 "#22332A" "#C8FFD0"
Text "Sharper silhouette and cockpit" ($aiX + 24) ($mainY + 462) 11 "#AAB5C4"; Chip "change" ($aiX + 300) ($mainY + 455) 58
Text "Keep mobile texture budget" ($aiX + 24) ($mainY + 496) 11 "#AAB5C4"; Chip "128 KB" ($aiX + 300) ($mainY + 489) 58 "#22332A" "#C8FFD0"

FillRound ($aiX + 12) ($mainY + 524) ($aiW - 24) 146 8 "#252B34" "#3B4552"
Text "Validation / Quality Gate" ($aiX + 24) ($mainY + 536) 12 "#E7EDF4" ([System.Drawing.FontStyle]::Bold)
Text "Style Bible match" ($aiX + 24) ($mainY + 568) 11 "#AAB5C4"; Chip "pass" ($aiX + 306) ($mainY + 561) 52 "#22332A" "#C8FFD0"
Text "128 px readability" ($aiX + 24) ($mainY + 600) 11 "#AAB5C4"; Chip "pass" ($aiX + 306) ($mainY + 593) 52 "#22332A" "#C8FFD0"
Text "Collision bounds" ($aiX + 24) ($mainY + 632) 11 "#AAB5C4"; Chip "review" ($aiX + 296) ($mainY + 625) 62 "#3B3020" "#FFE0A5"
FillRound ($aiX + 24) ($mainY + 684) 160 34 6 "#2D3540" "#3B4552"
CenterText "Ask AI to refine" ($aiX + 24) ($mainY + 684) 160 34 12 "#E7EDF4"
FillRound ($aiX + 198) ($mainY + 684) 160 34 6 "#1F3A36" "#32C6A6"
CenterText "Apply revision" ($aiX + 198) ($mainY + 684) 160 34 12 "#C9FFF2"

# Bottom panels
Rect 0 $bottomY $W $bottomH "#171A20" "#3B4552"
Rect 0 $bottomY 250 $bottomH "#20242B" "#3B4552"
PanelTitle "Asset Sets" 0 $bottomY 250
$ty = $bottomY + 48
TreeItem "Red Faction Enemies" 12 $ty 226 $true "#32C6A6"; $ty += 34
TreeItem "Player Weapons" 12 $ty 226 $false "#5AA7FF"; $ty += 34
TreeItem "Explosion VFX" 12 $ty 226 $false "#F2B84B"; $ty += 34
TreeItem "HUD Theme" 12 $ty 226 $false "#A98CFF"; $ty += 34
TreeItem "Build Bundles" 12 $ty 226 $false "#748194"

Rect 250 $bottomY 960 $bottomH "#171A20" "#3B4552"
PanelTitle "Project Assets / Candidate Revisions" 250 $bottomY 960
AssetTile 270 785 "enemy_small_sprite" "#5AA7FF"
AssetTile 420 785 "enemy_medium_sprite" "#D94B5A"
AssetTile 570 785 "enemy_boss_sprite" "#F2B84B"
AssetTile 720 785 "impact_vfx" "#A98CFF"
AssetTile 870 785 "explosion_audio" "#32C6A6"
AssetTile 1020 785 "enemy_prefab" "#7F8B9C"

Rect 1210 $bottomY 390 $bottomH "#14171C" "#3B4552"
PanelTitle "Build / Validation Report" 1210 $bottomY 390
Text "[AssetGraph] enemy_medium_sprite -> 2 prefabs, 1 scene" 1224 782 11 "#AAB5C4"
Text "[QualityGate] style match passed, mobile budget passed" 1224 806 11 "#AAB5C4"
Text "[Impact] No DSL change required" 1224 830 11 "#AAB5C4"
Text "[BundlePlan] hot update: changed asset only" 1224 854 11 "#AAB5C4"
Text "[Next] Review collision bounds before applying revision" 1224 878 11 "#F2B84B"

$bmp.Save($OutputPath, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose()
$bmp.Dispose()
Write-Output $OutputPath
