import struct, sys
from upload_lua_udp import Transport, fc03, function_offset

p, d, s = map(int, sys.argv[1:4])
base = 14000 + (((p - 1) * 30 * 30 + (d - 1) * 30 + (s - 1)) * 4)
r = Transport().request(fc03(base, 4))
off = function_offset(r)
pos = off + 3 if off == 2 else off + 2
w = struct.unpack(">4H", r[pos:pos+8])
bits = (w[2] << 16) | w[3]
value = struct.unpack(">f", struct.pack(">I", bits))[0]
print(f"{p},{d},{s}: type={w[1]>>8} flags=0x{w[1]&255:02x} value={value} bits=0x{bits:08x}")
