"""Builds the U6002 data-area blob (header + payload) from the SDCC build of app/data/ee_data.c.

Usage: python mkeedata.py build/oc_tas/data/ee_data app/data/ee_layout.h build/oc_tas/oc_tas_data.bin
  (expects ee_data.bin and ee_data.map next to the first argument)

Format (app/data/ee_layout.h): "OCDA", layout version, 0, payload length (LE16), CRC-32 of the payload (LE32,
zlib / crc32fast), then the payload = struct ee_data. The blob is written at 0x1800 of U6002 (0x1800..0x1FFF).
The payload is checked before anything is written: struct layout from ee_layout.h, every descriptor length,
wTotalLength values and RANGE counts, so that a short C initialiser (silently zero-padded) cannot pass.
"""
import math
import re
import struct
import sys
import zlib
from pathlib import Path


def defines(*headers):
    d = {}
    for h in headers:
        for m in re.finditer(r"^#define\s+(\w+)\s+(.+?)\s*(?:/\*.*)?$", Path(h).read_text(encoding="utf-8"), re.M):
            d[m.group(1)] = m.group(2)
    return d


def value(d, expr):
    expr = expr.strip()
    while expr in d:
        expr = d[expr].strip()
    expr = re.sub(r"\b(\w+)u\b", r"\1", expr)
    expr = re.sub(r"\b([A-Z_][A-Z0-9_]*)\b", lambda m: str(value(d, m.group(1))) if m.group(1) in d else m.group(1), expr)
    return int(eval(expr, {"__builtins__": {}}))


def layout(header):
    hdr = Path(header)
    d = defines(hdr.parent.parent / "usb" / "descriptors.h", hdr)
    body = re.search(r"struct ee_data \{(.*?)\};", hdr.read_text(encoding="utf-8"), re.S).group(1)
    members, off = {}, 0
    for name, n in re.findall(r"uint8_t\s+(\w+)\[([^\]]+)\];", body):
        members[name] = (off, value(d, n))
        off += members[name][1]
    return d, members, off


def check_config(cfg, rep_len, name):
    assert cfg[1] == 2 and cfg[2] | cfg[3] << 8 == len(cfg), f"{name}: wTotalLength"
    i = 0
    while i < len(cfg):
        assert cfg[i] >= 2, f"{name}: zero-length descriptor at offset {i}"
        if cfg[i + 1] == 0x24 and cfg[i + 2] == 0x01 and cfg[i] == 9:      # AC header (UAC2: 9 bytes)
            ac_total, j = cfg[i + 6] | cfg[i + 7] << 8, i
            while j < i + ac_total:
                assert cfg[j + 1] == 0x24, f"{name}: AC class-specific block ends early at offset {j}"
                j += cfg[j]
            assert j == i + ac_total, f"{name}: AC header wTotalLength"
        i += cfg[i]
    assert i == len(cfg), f"{name}: descriptor chain does not end at wTotalLength"
    hid = [i for i in range(len(cfg) - 9) if cfg[i] == 9 and cfg[i + 1] == 0x21]
    assert len(hid) == 1 and cfg[hid[0] + 7] == rep_len, f"{name}: HID class descriptor / wDescriptorLength"
    ifs = sorted({cfg[i + 2] for i in range(len(cfg) - 9) if cfg[i] == 9 and cfg[i + 1] == 0x04})
    assert ifs == list(range(cfg[4])), f"{name}: interface numbers {ifs} vs bNumInterfaces {cfg[4]}"


def stock_pod_db(x):
    """Stock pod volume curve: DSP 0x1001B140 with object 0x10010564 (firmware/dsp volume_curve.c VC_STOCK_POD)."""
    bp, sl, n = [99, 35, 4, 0], [0.3, 1.1, 6.0], 2
    x = min(max(x, bp[n + 1]), bp[0])
    d, db = bp[0] - x, 0.0
    for i in range(n + 1):
        db -= sl[i] * d
        d -= bp[i] - bp[i + 1]
        if d < 1.0:
            break
    return max(db, -71.3)


def check(p, members):
    def part(name):
        o, n = members[name]
        return p[o:o + n]

    devices = [m for m in members if m.startswith("device")]
    for name in devices:
        dev = part(name)
        assert dev[0] == 18 and dev[1] == 1, f"{name}: device descriptor"
    for name in devices:
        dev = part(name)
        assert (dev[8] | dev[9] << 8, dev[10] | dev[11] << 8) == (0x05A7, 0x1020), f"{name}: VID:PID 05A7:1020"
        assert (dev[12] | dev[13] << 8) >= 0x0500, f"{name}: bcdDevice must stay >= 0x0500 (stock 0x0100)"
    rep = part("hid_report")
    assert rep[0] == 0x06 and rep[-1] == 0xC0, "hid_report"
    configs = [m for m in members if m.startswith("config")]
    assert configs, "no configuration descriptor"
    for c in configs:
        check_config(part(c), len(rep), c)
    for s in ("str_langid", "str_manufacturer", "str_product"):
        b = part(s)
        assert b[0] == len(b) and b[1] == 3, f"{s}"
    fr = part("freq_range")
    assert fr == b"\x01\x00" + (48000).to_bytes(4, "little") * 2 + bytes(4), "freq_range: 48 kHz only (0.5.1)"
    assert part("vol_range")[:2] == b"\x01\x00" and len(part("vol_range")) == 2 + 6, "vol_range"
    knob = part("knob_db")
    want = bytes(int(math.floor(-stock_pod_db(k) + 0.5)) for k in range(100))
    assert knob == want, "knob_db differs from the stock pod curve"
    # GET_DESCRIPTOR lookups (app/usb/usb_app.c), one per audio mode, adjacent from offset 0 (EE_DESC_TAB +
    # 43 x mode, asm): [type][index][length LE16][EEPROM address LE16] x 7, 0
    tabs = [m for m in members if m.startswith("desc_tab")]
    base = 0x1800 + 12
    for k, name in enumerate(tabs):
        tab = part(name)
        assert members[name][0] == 43 * k and len(tab) == 43, f"{name}: must be at offset {43 * k}, 43 bytes"
        assert tab[-1] == 0, f"{name}: end marker"
        types = set()
        for i in range(0, len(tab) - 1, 6):
            t, idx, ln, addr = tab[i], tab[i + 1], tab[i + 2] | tab[i + 3] << 8, tab[i + 4] | tab[i + 5] << 8
            d = p[addr - base:addr - base + ln]
            assert t and len(d) == ln, f"{name} entry {i // 6}: outside the payload"
            types.add((t, idx))
            if t in (0x01, 0x03, 0x21):
                assert d[0] == ln and d[1] == t, f"{name} entry {i // 6}: bLength / bDescriptorType"
            elif t == 0x02:
                assert d[1] == 2 and (d[2] | d[3] << 8) == ln, f"{name}: configuration"
            elif t == 0x22:
                assert d == rep, f"{name}: HID report"
        assert {(1, 0), (2, 0), (3, 0), (3, 1), (3, 2), (0x21, 0), (0x22, 0)} <= types, f"{name}: entries missing"


def main():
    base, header, out = sys.argv[1:4]
    d, members, size = layout(header)
    binary = Path(base + ".bin").read_bytes()
    mem_map = Path(base + ".map").read_text()
    addr = int(re.search(r"C:\s+([0-9A-F]{8})\s+_ee_data\s", mem_map).group(1), 16)
    payload = binary[addr:addr + size]
    assert len(payload) == size, f"ee_data: {len(payload)} bytes in the image, struct is {size}"
    check(payload, members)

    version = value(d, "EE_LAYOUT_VERSION")
    hdr_len = value(d, "EE_HDR_LEN")
    start, end = value(d, "EE_AREA_START"), value(d, "EE_AREA_END")
    blob = b"OCDA" + struct.pack("<BBHI", version, 0, size, zlib.crc32(payload)) + payload
    assert hdr_len == 12 and len(blob) <= end - start, f"data area too large: {len(blob)} > {end - start}"
    Path(out).write_bytes(blob)
    print(f"data area: {len(blob)} bytes at 0x{start:04x}..0x{start + len(blob) - 1:04x} "
          f"(of 0x{start:04x}..0x{end - 1:04x}), layout {version}, CRC-32 0x{zlib.crc32(payload):08x}")


if __name__ == "__main__":
    main()
