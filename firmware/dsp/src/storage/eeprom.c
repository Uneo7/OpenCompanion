/* Non-blocking I2C1 master for the U7001 EEPROM (see include/eeprom.h).
 * Polled (ICIMR = 0): I2C1 shares CPU interrupt 14 with the TAS link on I2C0, which must not be disturbed.
 * Clock: module clock = SYSCLK2 122.88 MHz / (PSC 11 + 1) = 10.24 MHz (SPRU175D: 6.7..13.3 MHz), and
 * SCL = 10.24 MHz / ((CLKL + 5) + (CLKH + 5)) = ~394 kHz with CLKL = CLKH = 8, inside the AT24C512 400 kHz
 * rating. (The stock firmware uses PSC = 0x0D, CLKL = CLKH = 3, i.e. ~550 kHz.) The C6000 master stretches SCL while it waits for us, so a slow main loop
 * only slows the transfer down. */
#include "eeprom.h"
#include "regs_i2c.h"
#include "bose_tables.h"

#define I(off)       I2C_REG(I2C1_BASE, off)
#define DEV_ADDR     0x50
#define OWN_ADDR     0x7F               /* unused (master only) */

#define MDR_STP      (1u << 11)
#define MDR_STT      (1u << 13)
#define STR_BB       (1u << 12)

#define MAX_RETRIES  2000               /* address NACK retries: covers the ~5 ms write cycle */
#define SPIN_LIMIT   5000000u           /* polls without progress before giving up */

enum { S_IDLE, S_START, S_TX, S_RX, S_STOP, S_NACK_STOP, S_FINISHED };

static int state = S_IDLE, result = EE_IDLE;
static int writing;
static uint16_t ee_addr, len, tx_total, tx_sent, rx_got;
static uint8_t *rd_dst;
static const uint8_t *wr_src;
static unsigned retries;
static uint32_t spin;

static void hw_reset(void)
{
    I(I2C_ICMDR) = 0;                   /* hold in reset while configuring */
    I(I2C_ICPSC) = 11;
    I(I2C_ICCLKL) = 8;
    I(I2C_ICCLKH) = 8;
    I(I2C_ICOAR) = OWN_ADDR;
    I(I2C_ICIMR) = 0;
    I(I2C_ICMDR) = I2C_MDR_IRS | I2C_MDR_FREE;
}

void eeprom_init(void)
{
    hw_reset();
    state = S_IDLE;
    result = EE_IDLE;
}

static int begin(void)
{
    retries = 0;
    spin = 0;
    result = EE_BUSY;
    state = S_START;
    return 0;
}

int eeprom_read(uint16_t addr, uint8_t *dst, uint16_t n)
{
    if (state != S_IDLE || n == 0 || (uint32_t)addr + n > 0x10000u) return -1;
    writing = 0;
    ee_addr = addr;
    rd_dst = dst;
    len = n;
    return begin();
}

int eeprom_write(uint16_t addr, const uint8_t *src, uint16_t n)
{
    if (state != S_IDLE || n == 0 || n > EE_PAGE) return -1;
    if ((addr / EE_PAGE) != ((addr + n - 1u) / EE_PAGE)) return -1;   /* must not wrap inside a page */
    if (!(addr >= EE_WRITE_LO && (uint32_t)addr + n <= EE_WRITE_HI) &&    /* profiles */
        addr / EE_PAGE != EE_HDR / EE_PAGE && addr / EE_PAGE != EE_STATE / EE_PAGE) return -1;
    if (bose_tables_is_data_page(addr)) return -1;                    /* graph data pages: host only */
    if (addr / EE_PAGE == EE_HDR / EE_PAGE) {                         /* header: invalidation only */
        uint16_t i;
        for (i = 0; i < n; i++) if (src[i]) return -1;
    }
    writing = 1;
    ee_addr = addr;
    wr_src = src;
    len = n;
    return begin();
}

static void finish(int r)
{
    if (r == EE_ERROR) hw_reset();
    result = r;
    state = S_FINISHED;
}

static int stalled(void)
{
    if (++spin < SPIN_LIMIT) return 0;
    finish(EE_ERROR);
    return 1;
}

static void progress(void) { spin = 0; }

static uint8_t tx_byte(uint16_t i)
{
    if (i == 0) return (uint8_t)(ee_addr >> 8);
    if (i == 1) return (uint8_t)ee_addr;
    return wr_src[i - 2];
}

/* Device did not acknowledge: send STOP, then retry the whole job (ACK polling). */
static void on_nack(void)
{
    I(I2C_ICMDR) |= MDR_STP;
    I(I2C_ICSTR) = I2C_NACK;
    state = S_NACK_STOP;
    progress();
}

int eeprom_poll(void)
{
    uint32_t s;
    switch (state) {
    case S_IDLE:
        return EE_IDLE;

    case S_START:
        if ((I(I2C_ICMDR) & (MDR_STP | I2C_MDR_MST)) || (I(I2C_ICSTR) & STR_BB)) { stalled(); break; }
        /* Module reset (nothing pending in ICDXR), then the first address byte is loaded into ICDXR BEFORE STT:
         * otherwise a byte left in ICDXR after an address NACK (ACK polling) is sent first by the next transfer,
         * shifting every address by one byte (same as boot/boot1/src/ee.c). */
        I(I2C_ICMDR) = 0;
        I(I2C_ICMDR) = I2C_MDR_IRS | I2C_MDR_FREE;
        I(I2C_ICSAR) = DEV_ADDR;
        tx_total = (uint16_t)(writing ? 2u + len : 2u);
        rx_got = 0;
        I(I2C_ICCNT) = tx_total;
        I(I2C_ICMDR) = I2C_MDR_IRS | I2C_MDR_FREE | I2C_MDR_MST | I2C_MDR_TRX;     /* no START yet */
        I(I2C_ICDXR) = tx_byte(0);
        tx_sent = 1;
        /* write: one transaction with STOP; read: address phase without STOP, then repeated START */
        I(I2C_ICMDR) = I2C_MDR_IRS | I2C_MDR_FREE | I2C_MDR_MST | I2C_MDR_TRX | MDR_STT | (writing ? MDR_STP : 0);
        state = S_TX;
        progress();
        break;

    case S_TX:
        s = I(I2C_ICSTR);
        if (s & I2C_AL) { finish(EE_ERROR); break; }
        if (s & I2C_NACK) { on_nack(); break; }
        if ((s & I2C_ICXRDY) && tx_sent < tx_total) {
            I(I2C_ICDXR) = tx_byte(tx_sent++);
            progress();
            break;
        }
        if (tx_sent == tx_total) {
            if (writing) {
                state = S_STOP;
                progress();
            } else if (s & I2C_ARDY) {                               /* address sent: repeated START */
                I(I2C_ICSTR) = I2C_ARDY;
                I(I2C_ICCNT) = len;
                I(I2C_ICMDR) = I2C_MDR_IRS | I2C_MDR_FREE | I2C_MDR_MST | MDR_STT | MDR_STP;
                state = S_RX;
                progress();
            } else {
                stalled();
            }
            break;
        }
        stalled();
        break;

    case S_RX:
        s = I(I2C_ICSTR);
        if (s & I2C_AL) { finish(EE_ERROR); break; }
        if (s & I2C_NACK) { on_nack(); break; }
        if (s & I2C_ICRRDY) {
            uint8_t b = (uint8_t)I(I2C_ICDRR);
            if (rx_got < len) rd_dst[rx_got] = b;
            rx_got++;
            if (rx_got >= len) state = S_STOP;
            progress();
            break;
        }
        stalled();
        break;

    case S_STOP:
        s = I(I2C_ICSTR);
        if (s & I2C_NACK) { on_nack(); break; }                     /* data byte refused */
        if ((I(I2C_ICMDR) & (MDR_STP | I2C_MDR_MST)) || (s & STR_BB)) { stalled(); break; }
        I(I2C_ICSTR) = I2C_SCD | I2C_ARDY;
        finish(EE_DONE);
        break;

    case S_NACK_STOP:
        if ((I(I2C_ICMDR) & MDR_STP) || (I(I2C_ICSTR) & STR_BB)) { stalled(); break; }
        if (++retries > MAX_RETRIES) { finish(EE_ERROR); break; }
        I(I2C_ICSTR) = I2C_SCD | I2C_ARDY;
        state = S_START;
        break;

    case S_FINISHED:
    default:
        state = S_IDLE;
        return result;
    }
    return state == S_FINISHED ? EE_BUSY : (state == S_IDLE ? EE_IDLE : EE_BUSY);
}

int eeprom_busy(void) { return state != S_IDLE; }

int eeprom_read_wait(uint16_t addr, uint8_t *dst, uint16_t n)
{
    int r;
    if (eeprom_read(addr, dst, n)) return 0;
    do r = eeprom_poll(); while (r == EE_BUSY);
    return r == EE_DONE;
}
