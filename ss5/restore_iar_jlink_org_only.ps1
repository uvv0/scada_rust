$ErrorActionPreference = "Stop"
$iarBin = "C:\Program Files\IAR Systems\Embedded Workbench 9.1\arm\bin"
Copy-Item -LiteralPath (Join-Path $iarBin "JLinkARM_org.dll") `
          -Destination (Join-Path $iarBin "JLinkARM.dll") -Force
