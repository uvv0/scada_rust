param(
    [Parameter(Mandatory = $true)]
    [string]$TargetPath,

    [Parameter(Mandatory = $true)]
    [string]$OutputDir,

    [Parameter(Mandatory = $true)]
    [string]$Ielftool
)

$ErrorActionPreference = 'Stop'
$objectPayloads = @(
    @{
        Index = 2
        Start = [Convert]::ToUInt32('90005080', 16)
        End = [Convert]::ToUInt32('90005FFF', 16)
    }
)
$internalStart = [Convert]::ToUInt32('08000000', 16)
$internalEnd = [Convert]::ToUInt32('0801FFFF', 16)
$internalBinaryPath = Join-Path $OutputDir 'internal_flash.bin'
$internalHexPath = Join-Path $OutputDir 'internal_flash.hex'
$luaStart = [Convert]::ToUInt32('9002E080', 16)
$luaEnd = [Convert]::ToUInt32('9007FFFF', 16)
$luaBinaryPath = Join-Path $OutputDir 'lua_vm.bin'

foreach ($staleName in @('module_slot0.bin', 'module_slot0.hex',
                         'module_slot1.bin', 'module_slot1.hex')) {
    $stalePath = Join-Path $OutputDir $staleName
    if (Test-Path -LiteralPath $stalePath) {
        Remove-Item -LiteralPath $stalePath
    }
}

<#
 # Формирует одну строку Intel HEX и проверочную сумму.
 #>
function New-IntelHexRecord {
    param(
        [byte]$Type,
        [uint16]$Address,
        [byte[]]$Data
    )

    $sum = [int]$Data.Length
    $sum += ($Address -shr 8) -band 0xFF
    $sum += $Address -band 0xFF
    $sum += $Type
    $payload = foreach ($value in $Data) {
        $sum += $value
        '{0:X2}' -f $value
    }
    $checksum = (-$sum) -band 0xFF
    return ':{0:X2}{1:X4}{2:X2}{3}{4:X2}' -f `
        $Data.Length, $Address, $Type, ($payload -join ''), $checksum
}

<#
 # Извлекает ровно один 4-КБ слот из ELF-файла IAR.
 #>
function Export-ModuleBinary {
    param(
        [uint32]$SlotStart,
        [uint32]$SlotEnd,
        [string]$BinaryPath
    )

    & $Ielftool $TargetPath $binaryPath `
        --fill ('0xFF;0x{0:X8}-0x{1:X8}' -f $slotStart, $slotEnd) `
        ('--bin=0x{0:X8}-0x{1:X8}' -f $slotStart, $slotEnd)
    if ($LASTEXITCODE -ne 0) {
        throw "ielftool failed with code $LASTEXITCODE"
    }

    $length = (Get-Item -LiteralPath $binaryPath).Length
    if ($length -ne 4096) {
        throw "Unexpected module size: $length bytes"
    }
}

<#
 # Вычисляет CRC16 Modbus по заголовку без поля crc и по телу модуля.
 #>
function Set-ModuleCrc {
    param(
        [string]$BinaryPath
    )

    $binary = [System.IO.File]::ReadAllBytes($binaryPath)
    $address = [int]$binary[2] + ([int]$binary[3] -shl 8)
    $size = [int]$binary[4] + ([int]$binary[5] -shl 8)
    $imageEnd = if ($size -eq 0) { 4096 } else { $address + $size }

    if ($address -lt 10 -or $imageEnd -gt 4096 -or
        $imageEnd -le $address) {
        throw "Invalid module header: addr=$address size=$size"
    }

    $crc = 0xFFFF
    for ($index = 2; $index -lt $imageEnd; $index++) {
        $crc = $crc -bxor $binary[$index]
        for ($bit = 0; $bit -lt 8; $bit++) {
            if (($crc -band 1) -ne 0) {
                $crc = (($crc -shr 1) -bxor 0xA001) -band 0xFFFF
            }
            else {
                $crc = ($crc -shr 1) -band 0xFFFF
            }
        }
    }

    $binary[0] = [byte]($crc -band 0xFF)
    $binary[1] = [byte](($crc -shr 8) -band 0xFF)
    [System.IO.File]::WriteAllBytes($binaryPath, $binary)
    Write-Host ('Module CRC16: 0x{0:X4}' -f $crc)
}

<#
 # Преобразует 4-КБ BIN в Intel HEX с реальным XIP-адресом 0x90000000.
 #>
function Export-ModuleHex {
    param(
        [uint32]$SlotStart,
        [string]$BinaryPath,
        [string]$HexPath
    )

    $binary = [System.IO.File]::ReadAllBytes($binaryPath)
    $lines = [System.Collections.Generic.List[string]]::new()
    $upperValue = [int](($SlotStart -shr 16) -band 0xFFFF)
    $upper = [byte[]](
        [byte](($upperValue -shr 8) -band 0xFF),
        [byte]($upperValue -band 0xFF))
    $lowerBase = [int]($SlotStart -band 0xFFFF)

    $lines.Add((New-IntelHexRecord -Type 4 -Address 0 -Data $upper))
    for ($offset = 0; $offset -lt $binary.Length; $offset += 16) {
        $count = [Math]::Min(16, $binary.Length - $offset)
        $recordData = [byte[]]::new($count)
        [Array]::Copy($binary, $offset, $recordData, 0, $count)
        $lines.Add((New-IntelHexRecord -Type 0 `
            -Address ([uint16]($lowerBase + $offset)) -Data $recordData))
    }
    $lines.Add(':00000001FF')
    [System.IO.File]::WriteAllLines(
        $hexPath, $lines, [System.Text.Encoding]::ASCII)
}

<#
 # Извлекает внутреннюю Flash STM32 и создаёт Intel HEX без секций QSPI.
 # Этот файл используется C-SPY/J-Link как дополнительный download image.
 #>
function Export-InternalFlashHex {
    & $Ielftool $TargetPath $internalBinaryPath `
        --fill ('0xFF;0x{0:X8}-0x{1:X8}' -f `
            $internalStart, $internalEnd) `
        ('--bin=0x{0:X8}-0x{1:X8}' -f $internalStart, $internalEnd)
    if ($LASTEXITCODE -ne 0) {
        throw "ielftool internal Flash failed with code $LASTEXITCODE"
    }

    $binary = [System.IO.File]::ReadAllBytes($internalBinaryPath)
    if ($binary.Length -ne 131072) {
        throw "Unexpected internal Flash size: $($binary.Length) bytes"
    }

    $lines = [System.Collections.Generic.List[string]]::new()
    $lastUpper = -1
    for ($offset = 0; $offset -lt $binary.Length; $offset += 16) {
        $absolute = [uint64]$internalStart + [uint64]$offset
        $upperValue = [int](($absolute -shr 16) -band 0xFFFF)
        if ($upperValue -ne $lastUpper) {
            $upper = [byte[]](
                [byte](($upperValue -shr 8) -band 0xFF),
                [byte]($upperValue -band 0xFF))
            $lines.Add((New-IntelHexRecord `
                -Type 4 -Address 0 -Data $upper))
            $lastUpper = $upperValue
        }

        $count = [Math]::Min(16, $binary.Length - $offset)
        $recordData = [byte[]]::new($count)
        [Array]::Copy($binary, $offset, $recordData, 0, $count)
        $lines.Add((New-IntelHexRecord -Type 0 `
            -Address ([uint16]($absolute -band 0xFFFF)) `
            -Data $recordData))
    }
    $lines.Add(':00000001FF')
    [System.IO.File]::WriteAllLines(
        $internalHexPath, $lines, [System.Text.Encoding]::ASCII)
    Write-Host "Internal Flash HEX: $internalHexPath"
}

Export-InternalFlashHex
foreach ($object in $objectPayloads) {
    $binaryPath = Join-Path $OutputDir `
        ('module_slot{0}.bin' -f $object.Index)
    $legacyHexPath = Join-Path $OutputDir `
        ('module_slot{0}.hex' -f $object.Index)
    if (Test-Path -LiteralPath $legacyHexPath) {
        Remove-Item -LiteralPath $legacyHexPath
    }
    & $Ielftool $TargetPath $binaryPath `
        --fill ('0xFF;0x{0:X8}-0x{1:X8}' -f `
            $object.Start, $object.End) `
        ('--bin=0x{0:X8}-0x{1:X8}' -f $object.Start, $object.End)
    if ($LASTEXITCODE -ne 0) {
        throw "ielftool OBJ1 payload failed with code $LASTEXITCODE"
    }
    $length = (Get-Item -LiteralPath $binaryPath).Length
    if ($length -ne 3968) {
        throw "Unexpected OBJ1 payload size: $length bytes"
    }
    Write-Host "OBJ1 payload $($object.Index): $binaryPath"
}

& $Ielftool $TargetPath $luaBinaryPath `
    ('--bin=0x{0:X8}-0x{1:X8}' -f $luaStart, $luaEnd)
if ($LASTEXITCODE -ne 0) {
    throw "ielftool Lua payload failed with code $LASTEXITCODE"
}
$luaLength = (Get-Item -LiteralPath $luaBinaryPath).Length
if ($luaLength -le 0 -or
    $luaLength -gt ([uint64]$luaEnd - [uint64]$luaStart + 1)) {
    throw "Unexpected Lua payload size: $luaLength bytes"
}
Write-Host "Lua VM OBJ1 payload: $luaBinaryPath ($luaLength bytes)"
