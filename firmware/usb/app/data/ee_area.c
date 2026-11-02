/*
 * Boot check of the U6002 data area (data/ee_layout.h): header bytes 0..7 and the payload CRC-32 must match, else
 * ee_area_ok = 0 and the device never connects. Runs once, before the USB connection: the host tools power-cycle
 * the speaker after rewriting the area. CRC bitwise in assembly (SDCC's 32-bit C code is ~3x larger); one byte
 * per I2C read (~30 ms total): smallest code.
 */
#include "config.h"
#include "ee_area.h"
#include "ee_layout.h"
#include "i2c.h"
#include "stream.h"

#define DEV     (I2C_EEPROM | I2C_SUBADDR)
#define BUF     ((__xdata uint8_t *)EP2_IN_BUF)     /* scratch: the vendor HID is not configured yet */
#define HDR     (BUF + 16)                          /* header copy, out of the way of the byte reads at BUF[0] */
#define PAYLOAD (EE_AREA_START + EE_HDR_LEN)

_Static_assert(EE_MODE_ADDR + 1 == EE_AREA_START, "mode record read with the header");

__bit ee_area_ok;

__data uint8_t ee_crc[4];                           /* little-endian running CRC (crc_byte) */

static __code const uint8_t expected[8] = {
    EE_MAGIC, EE_LAYOUT_VERSION, 0,
    (uint8_t)sizeof(struct ee_data), (uint8_t)(sizeof(struct ee_data) >> 8),
};

/* ee_crc = CRC-32 update with the byte in DPL (reflected: LSB first). */
static void crc_byte(uint8_t b) __naked
{
    (void)b;
    __asm
        mov  a, dpl
        xrl  _ee_crc, a
        mov  r7, #8
    00001$:
        clr  c
        mov  a, (_ee_crc + 3)
        rrc  a
        mov  (_ee_crc + 3), a
        mov  a, (_ee_crc + 2)
        rrc  a
        mov  (_ee_crc + 2), a
        mov  a, (_ee_crc + 1)
        rrc  a
        mov  (_ee_crc + 1), a
        mov  a, _ee_crc
        rrc  a
        mov  _ee_crc, a
        jnc  00002$
        xrl  _ee_crc, #0x20
        xrl  (_ee_crc + 1), #0x83
        xrl  (_ee_crc + 2), #0xB8
        xrl  (_ee_crc + 3), #0xED
    00002$:
        djnz r7, 00001$
        ret
    __endasm;
}

void ee_area_check(void)
{
    static uint16_t addr;                       /* static: no push/pop around the calls (SDCC) */
    static uint8_t i;

    ee_area_ok = 0;
    if (!i2c_read(DEV, EE_MODE_ADDR, HDR - 1, 1 + EE_HDR_LEN)) {
        return;
    }
    if (HDR[-1] == MODE_CODE_21) {
        audio_mode = AUDIO_MODE_21;             /* else 5.1, the IDATA-clear value */
    }
    for (i = 0; i < 8; i++) {
        if (HDR[i] != expected[i]) {
            return;
        }
        ee_crc[i & 3] = 0xFF;                   /* init 0xFFFFFFFF */
    }
    for (addr = PAYLOAD; addr != PAYLOAD + sizeof(struct ee_data); addr++) {
        if (!i2c_read(DEV, addr, BUF, 1)) {
            return;
        }
        crc_byte(BUF[0]);
    }
    for (i = 0; i < 4; i++) {
        if ((uint8_t)~ee_crc[i] != HDR[8 + i]) {    /* final XOR */
            return;
        }
    }
    ee_area_ok = 1;
}
