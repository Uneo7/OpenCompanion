"""Prints the code size and RAM usage of a TAS1020B build from its SDCC outputs.

Usage: python size_report.py build/oc_tas/oc_tas      (expects .bin and .mem next to it)
"""
import re
import sys

CODE_RAM = 6016              # program RAM 0x0000..0x177F (DM SLES025B 6.1)
EP_SPACE = 1304              # endpoint buffer space 0xFA10..0xFF27 (DM 6.4.2)

# oc_tas buffer map (app/config.h); the XDATA variables are reported from the .mem file
EP_BUFFERS = [
    ("EP0 OUT", 8), ("EP0 IN", 8), ("EP2 IN vendor HID", 64), ("EP2 OUT vendor HID", 64),
    ("EP3 IN UAC2 interrupt", 8), ("EP1 OUT iso circular", 960), ("free", 96),
]


def main():
    base = sys.argv[1]
    code = len(open(base + ".bin", "rb").read())
    mem = open(base + ".mem").read()
    xram = re.search(r"EXTERNAL RAM\s+0x\w+\s+0x\w+\s+(\d+)\s+(\d+)", mem)
    stack = re.search(r"Stack starts at: (0x\w+).*with (\d+) bytes available", mem)
    print(f"code      {code} / {CODE_RAM} bytes ({CODE_RAM - code} free)")
    used = sum(n for _, n in EP_BUFFERS)
    print(f"endpoint  {used} bytes of buffers + {xram.group(2) if xram else 0} bytes reserved for XDATA variables"
          f" ({xram.group(1) if xram else '?'} used) = {used + int(xram.group(2) if xram else 0)} / {EP_SPACE}")
    for name, n in EP_BUFFERS:
        if n:
            print(f"  {name:22} {n}")
    if stack:
        print(f"stack     from {stack.group(1)}, {stack.group(2)} bytes of internal RAM")


if __name__ == "__main__":
    main()
