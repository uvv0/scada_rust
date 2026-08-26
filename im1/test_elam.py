#!/usr/bin/env python3
"""Test ELAM Modbus register read via UDP"""

import socket
import struct

TARGET = ("127.0.0.1", 5100)
STATION = 301
MODEM = 50002
REG_ADDR = 0

def crc16(data: bytes) -> int:
    crc = 0xFFFF
    for byte in data:
        crc ^= byte
        for _ in range(8):
            crc = (crc >> 1) ^ 0xA001 if crc & 1 else crc >> 1
    return crc

# Build ELAM header (22 bytes)
header = bytearray(22)
header[0] = 1                          # version
header[1] = 30                         # total_size LO (22 + 8)
header[2] = 0                          # total_size HI
header[3] = 1                          # packet_id
header[4] = 0                          # pkt_type: request
header[5] = 1                          # dsr LO
header[6] = 0                          # dsr HI
header[7] = MODEM & 0xFF               # modem LO
header[8] = (MODEM >> 8) & 0xFF        # modem HI
header[9] = 2                          # kan
header[10] = 8                         # speed
header[11] = 0                         # stop
header[12] = 2                         # par
header[13] = 3
header[14] = 3
header[15] = 0
header[16] = 30
header[17] = 0
header[18] = 255
header[19] = 0
header[20] = 255
header[21] = 0

# Build Modbus RTU read holding registers (func 03) - 8 bytes
modbus = bytearray(8)
modbus[0] = 1                          # station address
modbus[1] = 3                          # function: read holding registers
modbus[2] = (REG_ADDR >> 8) & 0xFF
modbus[3] = REG_ADDR & 0xFF
modbus[4] = 0                          # quantity HI
modbus[5] = 1                          # quantity LO: read 1 register
crc = crc16(modbus[:6])
modbus[6] = crc & 0xFF
modbus[7] = (crc >> 8) & 0xFF

packet = bytes(header) + bytes(modbus)

print(f"TX ({len(packet)} bytes): {packet.hex(' ')}")

sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.settimeout(2.0)
sock.sendto(packet, TARGET)

try:
    data, addr = sock.recvfrom(4096)
    print(f"RX ({len(data)} bytes): {data.hex(' ')}")

    if len(data) >= 10:
        print(f"  ELAM: ver={data[0]} size={data[1] | (data[2]<<8)} pid={data[3]} type={data[4]}")
        if len(data) >= 19:
            modbus_resp = data[10:]
            if len(modbus_resp) >= 7 and modbus_resp[1] == 3:
                value = (modbus_resp[3] << 8) | modbus_resp[4]
                print(f"  Register {REG_ADDR} = {value} (0x{value:04X})")
except socket.timeout:
    print("TIMEOUT - no response in 2 seconds")
except Exception as e:
    print(f"ERROR: {e}")
finally:
    sock.close()
