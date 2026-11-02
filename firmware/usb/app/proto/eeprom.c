/*
 * Guarded EEPROM access for firmware updates. During an update the header DataType is 3: if interrupted, the ROM
 * boots its DFU in EEPROM mode (DataType 3 is accepted even with a wrong checksum).
 * Header (DM 2.2.2.3): [0] checksum = sum of 1..17, [1] 0x12, [2..3] 0x1234, [14] DataType, [16..17] code size BE.
 * Writes: only inside a session, only code area 0x0012..0x1791 or data area 0x1800..0x1FFF, one 32-byte page at
 * most (the AT24C64 address counter wraps inside the page), each verified. The header is written only by
 * ee_begin() and ee_commit(). The AT24C64 NACKs during its write cycle (tWR <= 5 ms): read-back retried 20 ms.
 */
#include "config.h"
#include "ee_layout.h"
#include "eeprom.h"
#include "i2c.h"
#include "tick.h"

#define DEV             (I2C_EEPROM | I2C_SUBADDR)
#define HEADER_LEN      18
#define CODE_START      0x0012
#define CODE_END        (CODE_START + 6016)     /* exclusive */
#define PAGE            32
#define POLL_MS         20

__bit ee_session;

#define REQ ((__xdata uint8_t *)HID_REQ_DATA)    /* vendor HID OUT buffer, owned until replied */
#define REP ((__xdata uint8_t *)HID_REP_DATA)

uint8_t ee_read(uint16_t addr, uint8_t n)
{
    /* n is 1..60: this also refuses every addr >= 0x2000 except 0xFFC5..0xFFFF, which wrap to 0x1FC5..0x1FFF
     * (the AT24C64 ignores the top 3 address bits): harmless for a read. */
    if (addr + n > 0x2000) {
        return EE_ERR_RANGE;
    }
    return i2c_read(DEV, addr, REP, n) ? 0 : EE_ERR_IO;
}

/* REP[0 .. n) == REQ[off .. off + n), parameters of write_verify(). In assembly for size; P2 = 0xFA (free). */
_Static_assert(HID_REQ_DATA == 0xFA63 && HID_REP_DATA == 0xFA24 && EE_ERR_VERIFY == 0x21, "constants of the asm");
static uint8_t same_as_written(void) __naked
{
    __asm
        mov  0xA0, #0xFA
        mov  a, #0x63                           ; REQ + off
        add  a, _write_verify_PARM_2
        mov  r1, a
        mov  r0, #0x24                          ; REP
        mov  r7, _write_verify_PARM_3           ; n >= 1
    00001$:
        movx a, @r0
        mov  b, a
        movx a, @r1
        cjne a, b, 00002$
        inc  r0
        inc  r1
        djnz r7, 00001$
        mov  dpl, #0
        ret
    00002$:
        mov  dpl, #0x21                         ; EE_ERR_VERIFY
        ret
    __endasm;
}

static uint8_t write_verify(uint16_t addr, uint8_t off, uint8_t n)
{
    static uint8_t ms;                          /* static: no push/pop around the calls (SDCC) */
    if (!i2c_write(DEV, addr, REQ + off, n)) {
        return EE_ERR_IO;
    }
    for (ms = 0; ms < POLL_MS; ms++) {
        delay_ms(1);
        if (i2c_read(DEV, addr, REP, n)) {
            return same_as_written();
        }
    }
    return EE_ERR_IO;
}

static uint8_t header_shape_ok(const __xdata uint8_t *h)
{
    return h[1] == 0x12 && h[2] == 0x12 && h[3] == 0x34;
}

/* The checksum is left as is: the ROM accepts a wrong checksum when DataType = 3 (ROM 0x9512) and starts its DFU in
 * EEPROM mode, the wanted behaviour for an interrupted update. */
uint8_t ee_begin(void)
{
    uint8_t r;
    if (!i2c_read(DEV, 0, REP, HEADER_LEN)) {
        return EE_ERR_IO;
    }
    if (!header_shape_ok(REP)) {
        return EE_ERR_HEADER;
    }
    REQ[0] = 3;                                 /* the key has been checked: reuse its buffer */
    r = write_verify(14, 0, 1);
    if (!r) {
        ee_session = 1;
    }
    return r;
}

uint8_t ee_write(uint16_t addr, uint8_t n)
{
    uint8_t hi, lo;

    if (!ee_session) {
        return EE_ERR_STATE;
    }
    /* 8-bit tests: within one page addr + n - 1 stays in the same 256-byte block, so they are exact. */
    _Static_assert(CODE_START == 0x12 && CODE_END == 0x1792 && EE_AREA_START == 0x1800 && EE_AREA_END == 0x2000 &&
                   PAGE == 32, "ee_write() range tests");
    hi = (uint8_t)(addr >> 8);
    lo = (uint8_t)addr;
    if (!n || (uint8_t)((lo & (PAGE - 1)) + n) > PAGE || hi >= 0x20 || (!hi && lo < 0x12) ||
        (hi == 0x17 && (uint8_t)(lo + n - 1) >= 0x92)) {
        return EE_ERR_RANGE;
    }
    return write_verify(addr, 2, n);
}

/* Code size is the host's responsibility (the writable code area already stops at 6016 bytes).
 * DataType = 1 is written last: the header only becomes bootable with this final one-byte write. */
uint8_t ee_commit(void)
{
    uint8_t r, i, sum = 0;

    if (!ee_session) {
        return EE_ERR_STATE;
    }
    for (i = 1; i < HEADER_LEN; i++) {
        sum += REQ[i];
    }
    if (!header_shape_ok(REQ) || REQ[0] != sum || REQ[14] != 1) {
        return EE_ERR_HEADER;
    }
    r = write_verify(0, 0, 14);
    if (!r) {
        r = write_verify(15, 15, 3);
    }
    if (!r) {
        r = write_verify(14, 14, 1);
    }
    if (!r) {
        ee_session = 0;
    }
    return r;
}

/* No session: one byte outside the code and data areas; a torn write leaves a byte other than 2 (= mode 5.1). */
uint8_t ee_write_mode(void)
{
    return write_verify(EE_MODE_ADDR, 4, 1);
}

void ee_end(void)
{
    ee_session = 0;                             /* the header keeps DataType 3: next boot = ROM DFU */
}
