from upload_lua_udp import Transport, read_status

status = read_status(Transport())
print("state={state} result=0x{result:04x} object={id}".format(**status))
