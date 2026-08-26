import sys
from upload_lua_udp import Transport, fc16, fc06, write, wait_status, OBJECT_ID_HI, COMMAND

object_id = int(sys.argv[1], 0)
t = Transport()
write(t, fc16(OBJECT_ID_HI, (object_id >> 16, object_id & 0xffff)))
write(t, fc06(COMMAND, 0xA525))
s = wait_status(t, {6, 9}, timeout=15)
print(s)
