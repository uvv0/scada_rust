$ErrorActionPreference = "Stop"
$source = "C:\Program Files\SEGGER\JLink\JLinkARM.dll"
$target = "C:\Program Files\IAR Systems\Embedded Workbench 9.1\arm\bin\JLinkARM.dll"
Copy-Item -LiteralPath $source -Destination $target -Force
