$ErrorActionPreference = "Stop"
$iarBin = "C:\Program Files\IAR Systems\Embedded Workbench 9.1\arm\bin"
$active = Join-Path $iarBin "JLinkARM.dll"
$original = Join-Path $iarBin "JLinkARM_org.dll"
$backup = Join-Path $iarBin "JLinkARM_8.12f.before_restore.backup"

Copy-Item -LiteralPath $active -Destination $backup -Force
Copy-Item -LiteralPath $original -Destination $active -Force
