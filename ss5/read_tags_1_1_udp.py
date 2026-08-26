import struct
from upload_lua_udp import Transport, fc03, function_offset

r = Transport().request(fc03(14000, 120))
off = function_offset(r)
count = ((r[off + 1] << 8) | r[off + 2]) if off == 2 else r[off + 1]
pos = off + 3 if off == 2 else off + 2
if r[off] != 3 or count != 240:
    raise SystemExit(f"bad response: {r.hex(' ')}")
w = struct.unpack(">120H", r[pos:pos + count])
for sensor in range(1, 31):
    i = (sensor - 1) * 4
    tag_id, type_flags = w[i], w[i + 1]
    bits = (w[i + 2] << 16) | w[i + 3]
    if sensor <= 4 or type_flags or bits:
        print(f"1,1,{sensor}: id=0x{tag_id:04x} type={type_flags>>8} flags=0x{type_flags&255:02x} bits=0x{bits:08x}")
