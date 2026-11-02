"""Compares the RAM produced by test/host_image.c with the firmware ELF: every loadable byte must match, every
zero-filled (.bss/.far/.stack) byte must be 0, and nothing outside them may have been written (poison 0xA5).
Usage: py -3 -I check_ram.py firmware.out ram.bin"""
import struct
import sys

RAM_LO = 0x10000000


def segments(elf):
    phoff, = struct.unpack_from("<I", elf, 28)
    phentsize, phnum = struct.unpack_from("<HH", elf, 42)
    for i in range(phnum):
        typ, off, vaddr, _paddr, filesz, memsz, _fl, _al = struct.unpack_from("<8I", elf, phoff + i * phentsize)
        if typ == 1 and memsz:
            yield vaddr, elf[off:off + filesz], memsz


def main():
    elf = open(sys.argv[1], "rb").read()
    ram = bytearray(open(sys.argv[2], "rb").read())
    expect = bytearray(b"\xA5" * len(ram))
    for vaddr, data, memsz in segments(elf):
        o = vaddr - RAM_LO
        expect[o:o + len(data)] = data
        expect[o + len(data):o + memsz] = bytes(memsz - len(data))
    bad = [i for i in range(len(ram)) if ram[i] != expect[i]]
    if bad:
        print(f"MISMATCH: {len(bad)} bytes, first at 0x{RAM_LO + bad[0]:08X}")
        return 1
    print(f"RAM matches the ELF exactly ({len(ram)} bytes compared)")
    return 0


sys.exit(main())
