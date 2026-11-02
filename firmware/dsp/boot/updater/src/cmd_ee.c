/* Updater EEPROM commands ('E' sub-commands, little-endian; replies [06 ...] or [15 code]).
 *   10 addr:u16 n:u8 (1..60)            -> 06 data
 *   11 addr:u16 data (1..56 bytes)      -> 06       one page at most, inside the write window, read back
 *   12 addr:u16 len:u32                 -> 06 crc32 (zlib CRC-32 of the EEPROM bytes)
 *   13 'U' 'N' 'L' 'K'                  -> 06       migration only: 11 / 14 may then write boot0 and boot1
 *   14 ram:u32 crc:u32                  -> 06       migration only: boot0 (256 bytes staged in RAM) -> 0x0000
 * Codes: 1 refused (range / window / locked), 2 I2C error, 3 read-back mismatch, 4 staged data CRC mismatch.
 * A page write takes < 10 ms (transfer + tWR 5 ms + read-back), inside the 20 ms the TAS waits before reading
 * the reply of an 'E' command (doc.md). */
#include "boot.h"
#include "rescue.h"
#include "ee.h"
#include "crc32.h"
#include "layout.h"
#include "link.h"

extern int g_ee_unlock_boot;

static unsigned status(uint8_t *out, int r)
{
    out[0] = r ? NAK : ACK;
    out[1] = (uint8_t)r;
    return r ? 2 : 1;
}

static unsigned ee_crc(uint32_t a, uint32_t n, uint8_t *out)
{
    static uint8_t buf[EE_PAGE];
    uint32_t c = CRC32_INIT;
    if (a >= EE_SIZE || n > EE_SIZE - a) return status(out, EE_ERR_RANGE);
    while (n) {
        uint32_t k = n > EE_PAGE ? EE_PAGE : n;
        int r = ee_read(a, buf, k);
        if (r) return status(out, r);
        c = crc32_update(c, buf, k);
        a += k; n -= k;
    }
    out[0] = ACK;
    wr32(out + 1, ~c);
    return 5;
}

/* The only irrecoverable step of the migration: both boot0 pages are written back to back from RAM (no host
 * round trip in between), each read back by ee_write.
 * Window ~ 2 x tWR + one read-back + one page transfer (<= ~17 ms, docs/UPDATE.md). */
static unsigned write_boot0(uint32_t ram, uint32_t crc, uint8_t *out)
{
    const uint8_t *src = (const uint8_t *)ram;
    int r;
    if (!g_ee_unlock_boot || ram < APP_RAM_LO || ram > APP_RAM_HI - 256u) return status(out, EE_ERR_RANGE);
    if (crc32(src, 256) != crc) return status(out, 4);
    r = ee_write(EE_BOOT0, src, EE_PAGE);
    if (!r) r = ee_write(EE_BOOT0 + EE_PAGE, src + EE_PAGE, EE_PAGE);
    return status(out, r);
}

unsigned cmd_ext(const uint8_t *m, unsigned len, uint8_t *out)
{
    uint32_t a = len >= 3 ? rd16(m + 1) : 0, n;
    int r;
    switch (m[0]) {
    case RC_EE_READ:
        if (len < 4 || (n = m[3]) == 0 || n > LINK_MAX_RESP - 1) return status(out, EE_ERR_RANGE);
        r = ee_read(a, out + 1, n);
        if (r) return status(out, r);
        out[0] = ACK;
        return n + 1;
    case RC_EE_WRITE:
        if (len < 4) return status(out, EE_ERR_RANGE);
        return status(out, ee_write(a, m + 3, len - 3));
    case RC_EE_CRC:
        if (len < 7) return status(out, EE_ERR_RANGE);
        return ee_crc(a, rd32(m + 3), out);
    case RC_EE_UNLOCK_BOOT:
        if (len < 5 || m[1] != 'U' || m[2] != 'N' || m[3] != 'L' || m[4] != 'K') return status(out, EE_ERR_RANGE);
        g_ee_unlock_boot = 1;
        return status(out, EE_OK);
    case RC_EE_WRITE_BOOT0:
        if (len < 9) return status(out, EE_ERR_RANGE);
        return write_boot0(rd32(m + 1), rd32(m + 5), out);
    default:
        return 0;
    }
}
