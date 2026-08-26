import sys
from upload_lua_udp import Transport, fc16, fc06, fc03, write, wait_status, function_offset, OBJECT_ID_HI, COMMAND

slot = int(sys.argv[1]) if len(sys.argv) > 1 else 1
object_id = 0x4C530000 + slot
t = Transport()
write(t, fc16(OBJECT_ID_HI, (object_id >> 16, object_id & 0xffff)))
write(t, fc06(COMMAND, 0xA52A))
s = wait_status(t, {6, 12}, timeout=5)
if s['state'] != 12:
    raise SystemExit(s)
size = s['written']
out = bytearray()
for byte_off in range(0, size, 250):
    n = min(250, size - byte_off)
    r = t.request(fc03(10000 + byte_off // 2, (n + 1) // 2))
    off = function_offset(r)
    count = ((r[off+1] << 8) | r[off+2]) if off == 2 else r[off+1]
    pos = off + 3 if off == 2 else off + 2
    out += r[pos:pos+count]
print(bytes(out[:size]).decode('utf-8', errors='replace'))
