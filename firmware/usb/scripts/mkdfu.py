"""Wraps a TAS1020B application binary with the 18-byte boot header (Data Manual SLES025B, table 2-1).

The output is the image sent by a DFU download. It targets RAM: Attributes bit 1 ("write the header to
the EEPROM") is always 0, so the EEPROM header is left untouched.

Usage: python mkdfu.py firmware.bin firmware.dfu [--vid 0x05A7 --pid 0x1020]
"""
import argparse
import struct
import sys

HEADER_SIZE = 18
SIGNATURE = 0x1234
RAM_SIZE = 6016                 # program RAM, 0x0000..0x177F

DATA_TYPE_APPLICATION = 0x01
USB_ATTR_SELF_POWERED = 0x04
ATTR_400KHZ_I2C = 0x08          # bit 1 (header overwrite) deliberately 0


def build_header(payload_size, vid, pid, product_version, firmware_version):
    body = struct.pack(
        ">BHHHBBBBBBBBH",
        HEADER_SIZE,
        SIGNATURE,
        vid,
        pid,
        product_version,
        firmware_version,
        USB_ATTR_SELF_POWERED,
        1,                      # MaxPower, 2 mA units (same as the stock header)
        ATTR_400KHZ_I2C,
        0x20,                   # I2C write page size (AT24C64)
        DATA_TYPE_APPLICATION,
        0x00,                   # read page size: whole payload in one read
        payload_size,
    )
    # VID/PID/sizes are big-endian in the stock header (05 A7 10 20 ... 15 E4).
    checksum = sum(body) & 0xFF
    return bytes([checksum]) + body


def main():
    ap = argparse.ArgumentParser(description="Build a TAS1020B DFU (RAM) image")
    ap.add_argument("input")
    ap.add_argument("output")
    ap.add_argument("--vid", type=lambda s: int(s, 0), default=0x05A7, help="VID written in the header")
    ap.add_argument("--pid", type=lambda s: int(s, 0), default=0x1020, help="PID written in the header")
    ap.add_argument("--product-version", type=lambda s: int(s, 0), default=0x0A)
    ap.add_argument("--firmware-version", type=lambda s: int(s, 0), default=0xF0)
    args = ap.parse_args()

    payload = open(args.input, "rb").read()
    if len(payload) > RAM_SIZE:
        sys.exit(f"error: payload is {len(payload)} bytes, program RAM holds {RAM_SIZE}")
    header = build_header(len(payload), args.vid, args.pid, args.product_version, args.firmware_version)
    assert len(header) == HEADER_SIZE
    with open(args.output, "wb") as f:
        f.write(header + payload)
    print(f"{args.output}: header {HEADER_SIZE} B + payload {len(payload)} B (RAM limit {RAM_SIZE})")


if __name__ == "__main__":
    main()
