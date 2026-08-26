from upload_lua_udp import Transport, fc16, fc06, write, wait_status, OBJECT_ID_HI, COMMAND

t = Transport()
import sys
slot = int(sys.argv[1]) if len(sys.argv) > 1 else 1
object_id = 0x4C530000 + slot
write(t, fc16(OBJECT_ID_HI, (object_id >> 16, object_id & 0xffff)))
write(t, fc06(COMMAND, 0xA527))
s = wait_status(t, {6, 12}, timeout=5.0)
print(s)
