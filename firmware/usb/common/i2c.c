/*
 * I2C master, port of the stock I2cWrite 0x0E10 / I2cRead 0x001E / I2cWaitStatus 0x0F6C / I2cClearStatus 0x00EC.
 * STPWR is set before the last written byte, STPRD before reading the second-to-last byte (or the single one);
 * ERR = no acknowledge (DM 6.5.6.4). 400 kHz, as the stock inherits from the ROM boot (header Attributes.3).
 */
#include "i2c.h"
#include "tas1020b.h"

#define WAIT_LOOPS 20000u          /* > 10 ms at 24 MHz; one byte takes ~23 us at 400 kHz */

/* Stock I2cClearStatus: keep RXIE, FRQ, TXIE; clear ERR, STPRD, STPWR. */
static void clear_status(void)
{
    I2CCTL &= 0x54;
}

/* Stock I2cWaitStatus: wait for `flag` or ERR; on error or timeout, clear and force a stop. */
static uint8_t wait_flag(uint8_t flag)
{
    uint16_t n = WAIT_LOOPS;
    uint8_t st;
    do {
        st = I2CCTL;
        if (st & (flag | I2CCTL_ERR)) {
            if (!(st & I2CCTL_ERR)) {
                return 1;
            }
            break;
        }
    } while (--n);
    clear_status();
    I2CCTL |= I2CCTL_STPWR;
    clear_status();
    return 0;
}

void i2c_init(void)
{
    I2CCTL = I2CCTL_FRQ;
}

static uint8_t send_byte(uint8_t b)
{
    I2CDATO = b;
    return wait_flag(I2CCTL_TXE);
}

static uint8_t send_subaddr(uint8_t dev, uint16_t sub)
{
    I2CADR = (uint8_t)(dev << 1);               /* write */
    return send_byte((uint8_t)(sub >> 8)) && send_byte((uint8_t)sub);
}

uint8_t i2c_write(uint8_t dev, uint16_t sub, const __xdata uint8_t *data, uint8_t len)
{
    clear_status();
    if (dev & I2C_SUBADDR) {
        if (!send_subaddr(dev & 0x7F, sub)) {
            return 0;
        }
    } else {
        I2CADR = (uint8_t)(dev << 1);
    }
    while (len--) {
        if (len == 0) {
            I2CCTL |= I2CCTL_STPWR;             /* stop after the last byte */
        }
        if (!send_byte(*data++)) {
            return 0;
        }
    }
    clear_status();
    return 1;
}

uint8_t i2c_read(uint8_t dev, uint16_t sub, __xdata uint8_t *data, uint8_t len)
{
    clear_status();
    if ((dev & I2C_SUBADDR) && !send_subaddr(dev & 0x7F, sub)) {
        return 0;
    }
    I2CADR = (uint8_t)((dev << 1) | 1);         /* (repeated) start, read */
    if (len == 1) {
        I2CCTL |= I2CCTL_STPRD;
    }
    I2CDATO = 0xFF;                             /* dummy write starts the read (stock 0x0089 / 0x00A6) */
    while (len) {
        if (!wait_flag(I2CCTL_RXF)) {
            return 0;
        }
        if (len == 2) {
            I2CCTL |= I2CCTL_STPRD;             /* stop after the next (last) byte */
        }
        *data++ = I2CDATI;
        len--;
    }
    clear_status();
    return 1;
}
