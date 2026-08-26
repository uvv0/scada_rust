# Test ELAM Modbus register read via UDP
# Sends a request to the im1 simulator and prints the response

$TargetIP = "127.0.0.1"
$TargetPort = 5100
$RegAddr = 0   # Register to read

# Build ELAM header (22 bytes)
$header = New-Object byte[] 22
$header[0] = 1                          # version
$size = 30                              # total packet size (22 + 8)
$header[1] = $size -band 0xFF           # size LO
$header[2] = ($size -shr 8) -band 0xFF  # size HI
$header[3] = 1                          # packet_id
$header[4] = 0                          # pkt_type: request
$header[5] = 1                          # dsr LO
$header[6] = 0                          # dsr HI
$header[7] = 0xB2                       # modem LO (50002)
$header[8] = 0xC3                       # modem HI
$header[9] = 2                          # kan
$header[10] = 8                         # speed
$header[11] = 0                         # stop
$header[12] = 2                         # par
$header[13] = 3
$header[14] = 3
$header[15] = 0
$header[16] = 30
$header[17] = 0
$header[18] = 255
$header[19] = 0
$header[20] = 255
$header[21] = 0

# Build Modbus RTU read holding registers (func 03) - 8 bytes
$modbus = New-Object byte[] 8
$modbus[0] = 1                          # station address
$modbus[1] = 3                          # function: read holding registers
$modbus[2] = ($RegAddr -shr 8) -band 0xFF
$modbus[3] = $RegAddr -band 0xFF
$modbus[4] = 0                          # quantity HI
$modbus[5] = 1                          # quantity LO: read 1 register

# CRC16 (Modbus)
function Get-ModbusCRC16 {
    param([byte[]]$data)
    $crc = 0xFFFF
    foreach ($b in $data) {
        $crc = $crc -bxor $b
        for ($i = 0; $i -lt 8; $i++) {
            if ($crc -band 1) {
                $crc = ($crc -shr 1) -bxor 0xA001
            } else {
                $crc = $crc -shr 1
            }
        }
    }
    return $crc
}

$crc = Get-ModbusCRC16 -data $modbus
$modbus[6] = $crc -band 0xFF
$modbus[7] = ($crc -shr 8) -band 0xFF

# Combine header + modbus
$packet = New-Object byte[] 30
[Array]::Copy($header, 0, $packet, 0, 22)
[Array]::Copy($modbus, 0, $packet, 22, 8)

Write-Host "Sending ELAM+Modbus request to $TargetIP`:$TargetPort"
Write-Host "  Register: $RegAddr"
Write-Host "  Packet ($($packet.Length) bytes): $(($packet | ForEach-Object { '{0:X2}' -f $_ }) -join ' ')"

# Send UDP and receive response
$endpoint = New-Object System.Net.IPEndPoint([System.Net.IPAddress]::Parse($TargetIP), $TargetPort)
$client = New-Object System.Net.Sockets.UdpClient
$client.Client.SendTimeout = 2000
$client.Client.ReceiveTimeout = 2000
$client.Connect($endpoint)
$bytesSent = $client.Send($packet, $packet.Length)
Write-Host "Sent $bytesSent bytes"

$remote = $endpoint
try {
    $received = $client.Receive([ref]$remote)
    Write-Host "Received $($received.Length) bytes from $remote"
} catch {
    Write-Host "TIMEOUT/ERROR: $_"
    $received = $null
}

if ($received) {
    Write-Host "Response ($($received.Length) bytes): $(($received | ForEach-Object { '{0:X2}' -f $_ }) -join ' ')"

    # Parse ELAM response header (10 bytes)
    if ($received.Length -ge 10) {
        $elamVer = $received[0]
        $elamSize = $received[1] -bor ($received[2] -shr 8)
        $elamPid = $received[3]
        $elamType = $received[4]
        Write-Host "  ELAM: ver=$elamVer size=$elamSize pid=$elamPid type=$elamType"

        # Parse Modbus response
        if ($received.Length -ge 19) {
            $modbusResp = $received[10..($received.Length-1)]
            $station = $modbusResp[0]
            $func = $modbusResp[1]
            $byteCount = $modbusResp[2]
            Write-Host "  Modbus: station=$station func=$func byteCount=$byteCount"
            if ($func -eq 3 -and $byteCount -ge 2) {
                $value = ($modbusResp[3] -shr 8) -bor $modbusResp[4]
                # Actually big-endian:
                $value = ($modbusResp[3] * 256) + $modbusResp[4]
                Write-Host "  Register value: $value (0x$($value.ToString('X4')))"
            }
        }
    }
}

$client.Close()
