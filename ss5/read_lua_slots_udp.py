import struct
from upload_lua_udp import Transport, fc03, function_offset

response = Transport().request(fc03(32000, 100))
off = function_offset(response)
count = ((response[off + 1] << 8) | response[off + 2]) if off == 2 else response[off + 1]
pos = off + 3 if off == 2 else off + 2
if response[off] != 3 or count != 200:
    raise SystemExit(f"bad response: {response.hex(' ')}")
words = struct.unpack(">100H", response[pos:pos + count])
mask = (words[2] << 16) | words[3]
print(f"version={words[0]} active={words[1]} mask=0x{mask:08x}")
for slot in range(32):
    state = words[4 + slot * 3]
    result = (words[5 + slot * 3] << 16) | words[6 + slot * 3]
    if result & 0x80000000:
        result -= 0x100000000
    if state:
        print(f"slot={slot + 1} state={state} result={result}")
