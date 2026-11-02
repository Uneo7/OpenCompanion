/* Blocking I2C1 master for U7001 (AT24C512, address 0x50, 2-byte memory addresses, 128-byte pages).
 * Clock: module clock 122.88 MHz / (11 + 1) = 10.24 MHz, SCL = 10.24 MHz / ((8 + 5) * 2) = ~394 kHz
 * (AT24C512: 400 kHz). Count mode; reads = address phase without STOP, repeated START, receive, STOP.
 * An address NACK means the EEPROM is still in its internal write cycle: the transfer is retried
 * ("ACK polling"), so a read issued right after a write waits for the write to finish.
 *
 * Transmit start: module reset (IRS 0 -> 1), ICMDR = MST | TRX (no STT), ICDXR = first address byte, then STT.
 * The first byte MUST be in ICDXR before STT (as Linux i2c-davinci): otherwise a byte left in ICDXR after an
 * address NACK (ACK polling during tWR) is sent first by the next transfer, shifting every address by one byte. */
#include "ee.h"
#include "hw.h"
#include "layout.h"

#define I(off)      REG(I2C1_BASE + (off))
#define DEV         0x50u
#define SPIN        2000000u    /* polls per wait (>> one byte time) */
#define RETRIES     1000u       /* address NACK retries: >= 25 us each, covers tWR = 5 ms with margin */
#define CHUNK       0x8000u     /* ICCNT is 16 bits */

enum { OK = 0, E_NACK = -1, E_FAIL = -2 };

static int wait_set(uint32_t mask)
{
    uint32_t n = SPIN, s;
    while (n--) {
        s = I(I2C_STR);
        if (s & STR_AL) return E_FAIL;
        if (s & STR_NACK) return E_NACK;
        if (s & mask) return OK;
    }
    return E_FAIL;
}

/* Bus idle: no STOP pending, master mode left (cleared by hardware after the STOP), bus not busy. */
static int wait_idle(void)
{
    uint32_t n = SPIN;
    while (n--)
        if (!(I(I2C_MDR) & (MDR_STP | MDR_MST)) && !(I(I2C_STR) & STR_BB)) return OK;
    return E_FAIL;
}

/* Module reset (SPRU175D section 6): status bits back to their defaults (ICXRDY = 1), nothing left pending in
 * the transmit path. Prescaler and clock dividers keep their values. Only called with the bus idle. */
static void module_reset(void)
{
    I(I2C_MDR) = 0;
    I(I2C_MDR) = MDR_IRS | MDR_FREE;
}

void ee_init(void)
{
    (void)wait_idle();          /* let a transfer started by the trampoline end with its STOP */
    I(I2C_MDR) = 0;
    I(I2C_PSC) = 11;
    I(I2C_CLKL) = 8;
    I(I2C_CLKH) = 8;
    I(I2C_OAR) = 0x7F;
    I(I2C_IMR) = 0;
    I(I2C_MDR) = MDR_IRS | MDR_FREE;
}

/* START + device address + 2 memory address bytes (high, then low). The high byte is written to ICDXR before
 * STT, so the first byte on the wire is always ours. On NACK: STOP, and the caller retries. */
static int start(uint32_t addr, uint32_t cnt, uint32_t mdr)
{
    int r;
    if (wait_idle()) return E_FAIL;
    module_reset();
    I(I2C_SAR) = DEV;
    I(I2C_CNT) = cnt;
    I(I2C_MDR) = MDR_IRS | MDR_FREE | MDR_MST | MDR_TRX;        /* master transmitter, no START yet */
    I(I2C_DXR) = (addr >> 8) & 0xFFu;                           /* first byte loaded before STT */
    I(I2C_MDR) = MDR_IRS | MDR_FREE | MDR_MST | MDR_TRX | MDR_STT | mdr;
    r = wait_set(STR_XRDY);                                     /* high byte moved to the shift register */
    if (r == OK) { I(I2C_DXR) = addr & 0xFFu; r = wait_set(STR_XRDY); }
    /* Low byte moved too: the device address (sent before the high byte) has been acknowledged, so an address
     * NACK during the write cycle is always seen here, inside the caller's retry loop. */
    if (r == E_NACK) {
        I(I2C_MDR) |= MDR_STP;
        I(I2C_STR) = STR_NACK;
    }
    return r;
}

static int fail(void)
{
    ee_init();
    return EE_ERR_IO;
}

static int read_chunk(uint32_t addr, uint8_t *dst, uint32_t n)
{
    unsigned tries = RETRIES;
    uint32_t i;
    int r;
    while ((r = start(addr, 2, 0)) == E_NACK && --tries) { }
    if (r != OK || wait_set(STR_ARDY) != OK) return fail();
    I(I2C_STR) = STR_ARDY;
    I(I2C_CNT) = n;
    I(I2C_MDR) = MDR_IRS | MDR_FREE | MDR_MST | MDR_STT | MDR_STP;   /* repeated START, receive, STOP */
    for (i = 0; i < n; i++) {
        if (wait_set(STR_RRDY) != OK) return fail();
        dst[i] = (uint8_t)I(I2C_DRR);
    }
    return wait_idle() == OK ? EE_OK : fail();
}

int ee_read(uint32_t addr, uint8_t *dst, uint32_t n)
{
    if (addr >= EE_SIZE || n > EE_SIZE - addr) return EE_ERR_RANGE;
    while (n) {
        uint32_t k = n > CHUNK ? CHUNK : n;
        int r = read_chunk(addr, dst, k);
        if (r) return r;
        addr += k; dst += k; n -= k;
    }
    return EE_OK;
}

#ifdef BOOT_UPDATER
int g_ee_unlock_boot;                           /* set by RC_EE_UNLOCK_BOOT (migration only) */
#endif

static int write_ok(uint32_t addr, uint32_t n)
{
    if (n == 0 || n > EE_PAGE || addr / EE_PAGE != (addr + n - 1u) / EE_PAGE) return 0;
#ifdef BOOT_UPDATER
    if (g_ee_unlock_boot)
        return addr + n <= EE_LAST_PAGE;        /* boot0 and boot1 too, never the last page */
#endif
    return addr >= EE_WR_LO && addr + n <= EE_WR_HI;
}

int ee_write(uint32_t addr, const uint8_t *src, uint32_t n)
{
    static uint8_t back[EE_PAGE];
    unsigned tries = RETRIES;
    uint32_t i;
    int r;
    if (!write_ok(addr, n)) return EE_ERR_RANGE;
    while ((r = start(addr, 2u + n, MDR_STP)) == E_NACK && --tries) { }
    if (r != OK) return fail();
    for (i = 0; i < n; i++) {
        if (wait_set(STR_XRDY) != OK) return fail();
        I(I2C_DXR) = src[i];
    }
    if (wait_idle() != OK || (I(I2C_STR) & STR_NACK)) return fail();
    r = ee_read(addr, back, n);                 /* ACK polling: waits for the write cycle */
    if (r) return r;
    for (i = 0; i < n; i++)
        if (back[i] != src[i]) return EE_ERR_VERIFY;
    return EE_OK;
}
