import binascii
import socket
import struct
import time
from pathlib import Path

HOST = "192.168.1.100"
PORT = 5100
STATION = 301
PAYLOAD_PATH = Path(r"D:\picoC\4\modules\lua_vm.bin")

DATA_BASE = 10000
TOTAL_HI = 12048
OFFSET_HI = 12052
COMMAND = 12055
STATUS = 12056
OBJECT_ID_HI = 12062

CMD_BEGIN = 0xA521
CMD_CHUNK = 0xA522
CMD_COMMIT = 0xA523
CMD_ABORT = 0xA524
CMD_DELETE = 0xA529

ST_IDLE, ST_ERASING, ST_READY, ST_WRITING = 0, 1, 2, 3
ST_CHUNK_OK, ST_COMPLETE, ST_ERROR, ST_ABORTED = 4, 5, 6, 7


def crc16(data):
    crc = 0xFFFF
    for byte in data:
        crc ^= byte
        for _ in range(8):
            crc = (crc >> 1) ^ (0xA001 if crc & 1 else 0)
    return crc & 0xFFFF


def station_prefix():
    value = STATION - 248
    return bytes((0xF8 | ((value >> 8) & 7), value & 0xFF))


def frame(body):
    result = station_prefix() + body
    return result + struct.pack("<H", crc16(result))


def fc03(address, count):
    return frame(bytes((3,)) + struct.pack(">HH", address, count))


def fc06(address, value):
    return frame(bytes((6,)) + struct.pack(">HH", address, value))


def fc16(address, values):
    packed = b"".join(struct.pack(">H", value) for value in values)
    return frame(bytes((16,)) + struct.pack(">HHB", address, len(values), len(packed)) + packed)


class Transport:
    def __init__(self):
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.sock.settimeout(4.0)
        self.ident = 0

    def request(self, modbus):
        total = 22 + len(modbus)
        header = bytearray(22)
        header[0] = 1
        header[1:3] = struct.pack("<H", total)
        header[3] = self.ident
        self.ident = (self.ident + 1) & 0xFF
        header[4] = 0
        header[5:7] = struct.pack("<H", 11111)
        header[7:9] = struct.pack("<H", 301)
        header[9:22] = bytes((3, 8, 0, 2, 3, 3, 0, 30, 0, 255, 0, 255, 0))
        self.sock.sendto(header + modbus, (HOST, PORT))
        raw, source = self.sock.recvfrom(4096)
        if source[0] != HOST or len(raw) < 13:
            raise RuntimeError(f"bad UDP response from {source}, {len(raw)} bytes")
        if raw[9] == 0xFF:
            code = raw[10] if len(raw) > 10 else -1
            raise RuntimeError(f"gateway error {code}")
        response = raw[10:]
        if len(response) < 5 or crc16(response[:-2]) != struct.unpack("<H", response[-2:])[0]:
            raise RuntimeError("bad Modbus CRC")
        return response


def function_offset(response):
    return 2 if response[0] & 0xF8 == 0xF8 else 1


def write(transport, request):
    response = transport.request(request)
    qoff, roff = function_offset(request), function_offset(response)
    if response[roff] & 0x80:
        raise RuntimeError(f"Modbus exception {response[roff + 1]}")
    if response[roff:roff + 5] != request[qoff:qoff + 5]:
        raise RuntimeError(f"bad write ACK: {response.hex(' ')}")


def read_status(transport):
    response = transport.request(fc03(STATUS, 12))
    off = function_offset(response)
    if response[off] != 3:
        raise RuntimeError(f"bad status response: {response.hex(' ')}")
    if off == 2:
        count = (response[off + 1] << 8) | response[off + 2]
        pos = off + 3
    else:
        count = response[off + 1]
        pos = off + 2
    if count != 24:
        raise RuntimeError(f"bad status length {count}")
    words = struct.unpack(">12H", response[pos:pos + 24])
    return {
        "state": words[0], "result": words[1],
        "written": (words[2] << 16) | words[3],
        "crc": (words[4] << 16) | words[5],
        "id": (words[6] << 16) | words[7],
        "first": words[8], "blocks": words[9]
    }


def wait_status(transport, accepted, timeout=40.0):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        status = read_status(transport)
        marker = (status["state"], status["result"], status["written"])
        if marker != last:
            print(f"  status={status['state']} result=0x{status['result']:04x} written={status['written']}", flush=True)
            last = marker
        if status["state"] in accepted:
            return status
        if status["state"] == ST_ERROR:
            raise RuntimeError(f"controller error 0x{status['result']:04x}")
        time.sleep(0.12)
    raise TimeoutError(f"status timeout, last={last}")


def make_image(payload):
    payload_crc = binascii.crc32(payload) & 0xFFFFFFFF
    generation = int(time.time()) & 0xFFFFFFFF
    name = b"lua_vm.bin"
    header = struct.pack(
        "<IHHHHIIIIHHII", 0x314A424F, 1, 128, 3, 3, 5,
        generation, len(payload), payload_crc, 5, 0, 0, 0x9002E080
    )
    header += name + bytes(40 - len(name)) + bytes(44)
    assert len(header) == 124
    header += struct.pack("<I", binascii.crc32(header) & 0xFFFFFFFF)
    return header + payload, payload_crc


def main():
    payload = PAYLOAD_PATH.read_bytes()
    image, payload_crc = make_image(payload)
    image_crc = binascii.crc32(image) & 0xFFFFFFFF
    print(f"Payload {len(payload)} bytes CRC32=0x{payload_crc:08x}")
    print(f"OBJ1 {len(image)} bytes CRC32=0x{image_crc:08x}, blocks={(len(image)+4095)//4096}")
    transport = Transport()

    print("Abort current operation")
    write(transport, fc06(COMMAND, CMD_ABORT))
    wait_status(transport, {ST_ABORTED, ST_IDLE, ST_COMPLETE})

    print("Select and delete object 5")
    write(transport, fc16(OBJECT_ID_HI, (0, 5)))
    write(transport, fc06(COMMAND, CMD_DELETE))
    deleted = wait_status(transport, {ST_COMPLETE, ST_ERROR})
    if deleted["state"] == ST_ERROR:
        print(f"  delete result 0x{deleted['result']:04x}, continuing")

    print("Begin")
    write(transport, fc16(TOTAL_HI, (len(image) >> 16, len(image) & 0xFFFF,
                                    image_crc >> 16, image_crc & 0xFFFF)))
    write(transport, fc06(COMMAND, CMD_BEGIN))
    wait_status(transport, {ST_READY}, timeout=60.0)

    for offset in range(0, len(image), 4096):
        chunk = image[offset:offset + 4096]
        padded = chunk + (b"\xff" if len(chunk) & 1 else b"")
        words = struct.unpack(f">{len(padded)//2}H", padded)
        print(f"Chunk offset={offset} size={len(chunk)}")
        for word_offset in range(0, len(words), 123):
            write(transport, fc16(DATA_BASE + word_offset, words[word_offset:word_offset + 123]))
        write(transport, fc16(OFFSET_HI, (offset >> 16, offset & 0xFFFF, len(chunk))))
        write(transport, fc06(COMMAND, CMD_CHUNK))
        status = wait_status(transport, {ST_CHUNK_OK})
        if status["written"] != offset + len(chunk):
            raise RuntimeError(f"written mismatch: {status['written']} != {offset + len(chunk)}")

    print("Commit")
    write(transport, fc06(COMMAND, CMD_COMMIT))
    status = wait_status(transport, {ST_COMPLETE}, timeout=60.0)
    if status["crc"] != image_crc:
        raise RuntimeError(f"CRC mismatch: controller=0x{status['crc']:08x}, expected=0x{image_crc:08x}")
    print(f"UPLOAD OK object={status['id']} first_block={status['first']} blocks={status['blocks']}")


if __name__ == "__main__":
    main()
