from upload_lua_udp import (
    Transport, fc16, fc06, write, wait_status,
    OBJECT_ID_HI, COMMAND, ST_ERROR,
)


def main():
    transport = Transport()
    write(transport, fc16(OBJECT_ID_HI, (0, 5)))
    write(transport, fc06(COMMAND, 0xA525))
    status = wait_status(transport, {9, ST_ERROR}, timeout=15.0)
    print(f"START RESULT state={status['state']} result=0x{status['result']:04x}")


if __name__ == "__main__":
    main()
