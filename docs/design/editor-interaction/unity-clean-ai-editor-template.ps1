Add-Type -AssemblyName System.Drawing

$OutputPath = Join-Path $PSScriptRoot "unity-clean-ai-editor-template.png"
$W = 1908
$H = 1028
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
function Text($s, $x, $y, $size = 12, $color = "#D0D0D0", $style = [System.Drawing.FontStyle]::Regular) {
  $font = F $size $style
  $g.DrawString($s, $font, (B $color), [single]$x, [single]$y)
  $font.Dispose()
}
function CenterText($s, $x, $y, $w, $h, $size = 12, $color = "#D0D0D0", $style = [System.Drawing.FontStyle]::Regular) {
  $font = F $size $style
  $fmt = [System.Drawing.StringFormat]::new()
  $fmt.Alignment = [System.Drawing.StringAlignment]::Center
  $fmt.LineAlignment = [System.Drawing.StringAlignment]::Center
  $rect = [System.Drawing.RectangleF]::new([single]$x, [single]$y, [single]$w, [single]$h)
  $g.DrawString($s, $font, (B $color), $rect, $fmt)
  $fmt.Dispose()
  $font.Dispose()
}
function Tab($text, $x, $y, $w, $active = $false) {
  DrawRect $x $y $w 22 ($(if ($active) { "#3A3A3A" } else { "#2D2D2D" })) "#222222"
  Text $text ($x + 8) ($y + 4) 12 ($(if ($active) { "#E8E8E8" } else { "#A8A8A8" }))
}
function Button($text, $x, $y, $w, $h = 18, $fill = "#3A3A3A", $color = "#D0D0D0") {
  DrawRect $x $y $w $h $fill "#242424"
  CenterText $text $x $y $w $h 11 $color
}
function TreeItem($text, $x, $y, $w, $active = $false, $indent = 0, $dim = $false) {
  if ($active) { DrawRect $x $y $w 17 "#2F6EA5" }
  $color = if ($active) { "#FFFFFF" } elseif ($dim) { "#777777" } else { "#C8C8C8" }
  Text $text ($x + 10 + $indent) ($y + 1) 12 $color
}
function Header($text, $x, $y, $w) {
  DrawRect $x $y $w 20 "#383838" "#252525"
  Text $text ($x + 6) ($y + 3) 12 "#D6D6D6"
}
function Field($label, $value, $x, $y, $w) {
  Text $label $x ($y + 3) 12 "#C2C2C2"
  DrawRect ($x + 110) $y ($w - 110) 18 "#252525" "#191919"
  Text $value ($x + 118) ($y + 2) 12 "#D8D8D8"
}
function Checkbox($label, $x, $y, $checked = $true) {
  DrawRect $x $y 13 13 "#242424" "#151515"
  if ($checked) { Text "x" ($x + 3) ($y - 2) 12 "#DCDCDC" }
  Text $label ($x + 20) ($y - 1) 12 "#C8C8C8"
}
function ConsoleRow($text, $x, $y, $w, $level = "error") {
  $fill = if ($level -eq "ai") { "#33413C" } else { "#444444" }
  DrawRect $x $y $w 38 $fill
  $iconColor = if ($level -eq "ai") { "#45C89B" } else { "#F64E4E" }
  $g.FillEllipse((B $iconColor), $x + 10, $y + 8, 22, 22)
  Text ($(if ($level -eq "ai") { "AI" } else { "!" })) ($x + 14) ($y + 8) 12 "#FFFFFF" ([System.Drawing.FontStyle]::Bold)
  Text $text ($x + 42) ($y + 5) 12 "#D6D6D6"
  if ($level -eq "ai") {
    Text "Review generated plan before applying." ($x + 42) ($y + 21) 11 "#AFCFC3"
  } else {
    Text "Your script should either check if it is null or should not destroy the object." ($x + 42) ($y + 21) 11 "#BDBDBD"
  }
}

DrawRect 0 0 $W $H "#303030"

# OS / Unity title area
DrawRect 0 0 $W 22 "#F7F7F7"
Text "Administrator: AI Game Engine - Game - Windows, Mac, Linux - Unity-like Workspace" 8 3 12 "#111111"
DrawRect 0 22 $W 22 "#F5F5F5"
Text "File   Edit   Assets   GameObject   Component   Services   Tools   AI   Window   Help" 4 25 12 "#000000"

# Toolbar
DrawRect 0 44 $W 28 "#2A2A2A" "#181818"
Button "Sign in" 4 49 58 18 "#3A3A3A"
Button "AI" 72 49 34 18 "#2F5A4B" "#D8FFF1"
Button "Ask" 112 49 42 18 "#3A3A3A"
Button "Scene" 560 49 68 18 "#3A3A3A"
Button "Game" 633 49 68 18 "#3A3A3A"
Button "AI Overlay" 708 49 86 18 "#2F5A4B" "#D8FFF1"
Button "Play" 910 49 30 18 "#3A3A3A"
Button "Pause" 944 49 30 18 "#3A3A3A"
Button "Step" 978 49 30 18 "#3A3A3A"
Text "play speed:" 1048 51 12 "#C0C0C0"
Button "1" 1218 49 32 18 "#063C0D" "#7CFF88"
Button "Layers" 1710 49 78 18 "#3A3A3A"
Button "Layout" 1830 49 74 18 "#3A3A3A"

$top = 72
$bottom = 590
$leftW = 555
$rightX = 1322
$rightW = 586
$centerX = $leftW
$centerW = $rightX - $centerX

# Hierarchy
DrawRect 0 $top $leftW ($bottom - $top) "#373737" "#1F1F1F"
Tab "Hierarchy" 0 $top 90 $true
DrawRect 0 ($top + 22) $leftW 18 "#2E2E2E" "#202020"
Text "+  v" 4 ($top + 25) 12 "#CFCFCF"
DrawRect 225 ($top + 24) 320 14 "#2A2A2A" "#202020"
Text "All" 242 ($top + 24) 11 "#AFAFAF"
$hy = $top + 44
TreeItem "v Game" 28 $hy 520 $false 0; $hy += 17
TreeItem "  o NullCamera" 28 $hy 520 $false 18; $hy += 17
TreeItem "  o MainUpdate" 28 $hy 520 $false 18; $hy += 17
TreeItem "  > UIRoot" 28 $hy 520 $false 18; $hy += 17
TreeItem "  o BattleDevelope" 28 $hy 520 $false 18 $true; $hy += 17
TreeItem "  o DynamicBoneJobManager" 28 $hy 520 $false 18 $true; $hy += 17
TreeItem "  o WorldDevelope" 28 $hy 520 $false 18 $true; $hy += 17
TreeItem "  o MultipathNetworkBinder" 28 $hy 520 $false 18 $true; $hy += 17
TreeItem "  o LogCollection" 28 $hy 520 $false 18; $hy += 17
TreeItem "  o CameraRoot" 28 $hy 520 $false 18; $hy += 17
TreeItem "  o Proxima" 28 $hy 520 $true 18; $hy += 17
TreeItem "  o AbInspector" 28 $hy 520 $false 18; $hy += 17
TreeItem "  o ScreenTools" 28 $hy 520 $false 18
DrawRect 526 ($top + 4) 22 18 "#333333" "#202020"
Text "AI" 531 ($top + 6) 10 "#D8FFF1"

# Center Game view
DrawRect $centerX $top $centerW ($bottom - $top) "#222222" "#1C1C1C"
Tab "Scene" $centerX $top 76 $false
Tab "Game" ($centerX + 76) $top 76 $true
Tab "Timeline" ($centerX + 152) $top 90 $false
DrawRect $centerX ($top + 22) $centerW 22 "#3A3A3A" "#202020"
Text "Game     Display 1     16:9 Aspect     Scale  1x     Play Focused     Stats  Gizmos" ($centerX + 6) ($top + 27) 12 "#D0D0D0"
Button "AI Preview" ($centerX + $centerW - 198) ($top + 24) 82 18 "#2F5A4B" "#D8FFF1"
Button "Validate" ($centerX + $centerW - 108) ($top + 24) 72 18 "#3A3A3A"
DrawRect $centerX ($top + 44) $centerW ($bottom - $top - 66) "#000000"
DrawRect $centerX ($bottom - 22) $centerW 22 "#252525"

# Inspector
DrawRect $rightX $top $rightW ($bottom - $top) "#383838" "#1F1F1F"
Tab "Inspector" $rightX $top 96 $true
Tab "AI" ($rightX + 96) $top 46 $false
Tab "ActorSkillWindow" ($rightX + 142) $top 132 $false
Text "o  x  Proxima" ($rightX + 24) ($top + 32) 13 "#D8D8D8" ([System.Drawing.FontStyle]::Bold)
DrawRect ($rightX + 165) ($top + 30) 350 18 "#262626" "#1A1A1A"
Text "Static" ($rightX + 520) ($top + 32) 12 "#BDBDBD"
Field "Tag" "Untagged" ($rightX + 70) ($top + 56) 300
Field "Layer" "Default" ($rightX + 350) ($top + 56) 230

Header "Transform" $rightX ($top + 82) $rightW
Field "P  X" "0" ($rightX + 24) ($top + 108) 180
Field "Y" "0" ($rightX + 235) ($top + 108) 160
Field "Z" "0" ($rightX + 410) ($top + 108) 160
Field "R  X" "0" ($rightX + 24) ($top + 130) 180
Field "Y" "0" ($rightX + 235) ($top + 130) 160
Field "Z" "0" ($rightX + 410) ($top + 130) 160
Field "S  X" "1" ($rightX + 24) ($top + 152) 180
Field "Y" "1" ($rightX + 235) ($top + 152) 160
Field "Z" "1" ($rightX + 410) ($top + 152) 160

Header "Proxima Inspector (Script)" $rightX ($top + 178) $rightW
Button "AI Fix" ($rightX + $rightW - 76) ($top + 180) 56 16 "#2F5A4B" "#D8FFF1"
Field "Display Name" "" ($rightX + 24) ($top + 204) 540
Field "Port" "7759" ($rightX + 24) ($top + 226) 540
Checkbox "Use Https" ($rightX + 24) ($top + 251) $false
Field "Password" "123456" ($rightX + 24) ($top + 274) 540
DrawRect ($rightX + 24) ($top + 302) 540 56 "#45413A" "#2A2A2A"
Text "!" ($rightX + 38) ($top + 314) 28 "#FFC72C"
Text "Setting a password here is not recommended." ($rightX + 76) ($top + 310) 12 "#D6D6D6"
Text "AI: Can generate a safer connection UI or remove hardcoded password." ($rightX + 76) ($top + 330) 12 "#D8FFF1"
Checkbox "Run On Enable" ($rightX + 24) ($top + 374) $true
Field "Log Buffer Size" "1000" ($rightX + 24) ($top + 396) 540
Checkbox "Instantiate Status UI" ($rightX + 24) ($top + 423) $false
Checkbox "Instantiate Connect UI" ($rightX + 24) ($top + 446) $false
Checkbox "Dont Destroy On Load" ($rightX + 24) ($top + 469) $true
Checkbox "Set Run In Background" ($rightX + 24) ($top + 492) $true
Button "Add Component" ($rightX + 184) ($top + 530) 228 22 "#555555"

# Bottom panels
DrawRect 0 $bottom $W ($H - $bottom) "#353535" "#1F1F1F"
$leftBottomW = 558
DrawRect 0 $bottom $leftBottomW ($H - $bottom) "#333333" "#1F1F1F"
Tab "Project" 0 $bottom 76 $true
DrawRect 0 ($bottom + 22) $leftBottomW 26 "#303030" "#202020"
Text "+  v" 4 ($bottom + 28) 12 "#CFCFCF"
DrawRect 90 ($bottom + 28) 315 15 "#272727" "#202020"
Text "Search" 98 ($bottom + 28) 11 "#AFAFAF"
$fy = $bottom + 48
$folders = @("ShiTaKe_001","ShiTaKeLang_001","ShiTaKeXuShan_001","ShiTianYuLong_001","SiFengYuanYeYi_001","SunWuKong_002","SunWuKong_003","SunWuKongChaoYi_001","SuoLong_001","TanZhiLang_001","TeLanKeSi_001","WoAiLuo_002","WuQiShanYi_001","WuDuWaWa_001","WuSuoPu_001","WuTiaoWu_001","WuTiaoWuCi_001","XiaoJie_001","XiaoMingRenFenShen_001","XiaoMingRenFenShen_002","XiSuo_001","XiSuoPai_001","XiSuoPai_002")
foreach ($f in $folders) {
  TreeItem "[D] $f" 88 $fy 218 $false 0
  $fy += 16
}
DrawRect 320 ($bottom + 44) 238 ($H - $bottom - 66) "#3A3A3A" "#252525"
Text "Assets > Game > GameAsset > Config" 330 ($bottom + 48) 12 "#BDBDBD"
$ay = $bottom + 70
foreach ($n in @("201","201_ActorSkillUseAssetConfig","201_config_skill","202","202_ActorSkillUseAssetConfig","202_config_skill","203","203_ActorSkillUseAssetConfig","203_config_skill","301","301_ActorSkillUseAssetConfig","301_config_skill")) {
  Text "▦ $n" 338 $ay 12 "#D2D2D2"
  $ay += 16
}

$consoleX = 558
$consoleW = 764
DrawRect $consoleX $bottom $consoleW ($H - $bottom) "#333333" "#1F1F1F"
Tab "Animation" $consoleX $bottom 98 $false
Tab "Animator" ($consoleX + 98) $bottom 96 $false
Tab "Console" ($consoleX + 194) $bottom 86 $true
Tab "AI Plan" ($consoleX + 280) $bottom 86 $false
DrawRect $consoleX ($bottom + 22) $consoleW 26 "#303030" "#202020"
Text "Clear   Collapse   Error Pause   Editor" ($consoleX + 6) ($bottom + 28) 12 "#CFCFCF"
Button "Ask AI About Errors" ($consoleX + $consoleW - 180) ($bottom + 26) 142 18 "#2F5A4B" "#D8FFF1"
ConsoleRow "[16:38:22] UnobservedTaskException: A Task's exception(s) were not observed." ($consoleX + 10) ($bottom + 52) ($consoleW - 20)
ConsoleRow "[16:38:22] StackTrace:" ($consoleX + 10) ($bottom + 92) ($consoleW - 20)
ConsoleRow "[16:38:22] MissingReferenceException: The object of type 'Transform' has been destroyed." ($consoleX + 10) ($bottom + 132) ($consoleW - 20)
ConsoleRow "[AI Plan] Found likely null Transform access. Generate patch plan?" ($consoleX + 10) ($bottom + 172) ($consoleW - 20) "ai"

DrawRect $rightX $bottom $rightW ($H - $bottom) "#383838" "#1F1F1F"
Tab "AI Plan Preview" $rightX $bottom 128 $true
Tab "Inspector Notes" ($rightX + 128) $bottom 130 $false
Text "Selection-aware AI panel, kept small and docked." ($rightX + 18) ($bottom + 44) 13 "#D8FFF1" ([System.Drawing.FontStyle]::Bold)
Text "Selected object: Proxima" ($rightX + 18) ($bottom + 74) 12 "#D6D6D6"
Text "Suggested action:" ($rightX + 18) ($bottom + 104) 12 "#C8C8C8"
Text "1. Remove hardcoded password or create runtime password UI." ($rightX + 36) ($bottom + 132) 12 "#C8C8C8"
Text "2. Add null check before Transform access." ($rightX + 36) ($bottom + 156) 12 "#C8C8C8"
Text "3. Re-run validation after patch." ($rightX + 36) ($bottom + 180) 12 "#C8C8C8"
Button "Generate Patch Plan" ($rightX + 18) ($bottom + 222) 150 24 "#2F5A4B" "#D8FFF1"
Button "Explain Error" ($rightX + 180) ($bottom + 222) 110 24 "#555555"
Button "Close" ($rightX + 302) ($bottom + 222) 78 24 "#555555"

# Bottom red status line like Unity console.
DrawRect 0 ($H - 18) $W 18 "#101010"
Text "! MissingReferenceException: The object of type 'Transform' has been destroyed but you are still trying to access it." 4 ($H - 18) 12 "#FF3E3E"
Text "AI ready" ($W - 90) ($H - 18) 12 "#D8FFF1"

$bmp.Save($OutputPath, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose()
$bmp.Dispose()
Write-Output $OutputPath

