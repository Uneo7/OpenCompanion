/*
 * 0x20 RESTORE_HEADER: puts the U6002 header back to the stock value, and nothing else.
 * Expected 6a 12 12 34 05 a7 10 20 0a 04 04 01 08 20 02 00 15 e4 (DataType 2), stock 69 ... 01 00 15 e4: only
 * byte 14 (DataType) and byte 0 (checksum) differ. Refuses anything but the expected header.
 * Byte 14 is written before byte 0: if stopped in between, DataType 1 with a wrong checksum makes the ROM start its
 * DFU in EEPROM mode, so the speaker stays recoverable.
 * AT24C64 acknowledge polling (tWR <= 5 ms): each poll is a one-byte read of the byte just written.
 * Reply: [0x20][18 bytes read back] or [0xA0][error][18 bytes read, if any].
 */
#include "board.h"
#include "i2c.h"
#include "restore_header.h"

#define EEPROM_DEV       (0x50 | I2C_SUBADDR)
#define HEADER_LEN       18
#define POLL_MAX_MS      20             /* 4 x tWR(max) */

#define ERR_READ         0x10           /* initial read failed: nothing written */
#define ERR_MISMATCH     0x11           /* header is not the expected one: nothing written */
#define ERR_WRITE_TYPE   0x12           /* write of byte 14 failed */
#define ERR_WRITE_SUM    0x13           /* byte 14 written, write of byte 0 failed */
#define ERR_VERIFY_READ  0x14           /* both written, read-back failed */
#define ERR_VERIFY       0x15           /* read-back differs from the stock header */

static __code const uint8_t expected_header[HEADER_LEN] = {
    0x6a, 0x12, 0x12, 0x34, 0x05, 0xa7, 0x10, 0x20, 0x0a, 0x04, 0x04, 0x01, 0x08, 0x20, 0x02, 0x00, 0x15, 0xe4
};
static __code const uint8_t stock_header[HEADER_LEN] = {
    0x69, 0x12, 0x12, 0x34, 0x05, 0xa7, 0x10, 0x20, 0x0a, 0x04, 0x04, 0x01, 0x08, 0x20, 0x01, 0x00, 0x15, 0xe4
};

static uint8_t equals(const __xdata uint8_t *a, __code const uint8_t *b)
{
    uint8_t i;
    for (i = 0; i < HEADER_LEN; i++) {
        if (a[i] != b[i]) {
            return 0;
        }
    }
    return 1;
}

/* Writes one byte, then acknowledge-polls until the EEPROM is ready and the byte reads back as written. */
static uint8_t write_byte(uint16_t addr, uint8_t value, __xdata uint8_t *scratch)
{
    uint8_t ms;
    scratch[0] = value;
    if (!i2c_write(EEPROM_DEV, addr, scratch, 1)) {
        return 0;
    }
    for (ms = 0; ms < POLL_MAX_MS; ms++) {
        delay_ms(1);
        if (i2c_read(EEPROM_DEV, addr, scratch, 1)) {
            return scratch[0] == value;
        }
    }
    return 0;
}

static uint8_t fail(__xdata uint8_t *out, uint8_t code, uint8_t with_header)
{
    out[0] = CMD_RESTORE_HEADER | 0x80;
    out[1] = code;
    return with_header ? 2 + HEADER_LEN : 2;
}

uint8_t restore_header(__xdata uint8_t *out)
{
    __xdata uint8_t *hdr = out + 2;             /* header bytes go straight into the reply */
    __xdata uint8_t *scratch = out + 2 + HEADER_LEN;

    i2c_init();
    if (!i2c_read(EEPROM_DEV, 0x0000, hdr, HEADER_LEN)) {
        return fail(out, ERR_READ, 0);
    }
    if (!equals(hdr, expected_header)) {
        return fail(out, ERR_MISMATCH, 1);
    }
    if (!write_byte(14, stock_header[14], scratch)) {
        return fail(out, ERR_WRITE_TYPE, 0);
    }
    if (!write_byte(0, stock_header[0], scratch)) {
        return fail(out, ERR_WRITE_SUM, 0);
    }
    if (!i2c_read(EEPROM_DEV, 0x0000, hdr, HEADER_LEN)) {
        return fail(out, ERR_VERIFY_READ, 0);
    }
    if (!equals(hdr, stock_header)) {
        return fail(out, ERR_VERIFY, 1);
    }
    out[0] = CMD_RESTORE_HEADER;
    out[1] = HEADER_LEN;
    return 2 + HEADER_LEN;
}
