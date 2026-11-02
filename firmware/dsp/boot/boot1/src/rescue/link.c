/* I2C0 slave 0x29, polled: the TAS frame layer of the RAM loader (../../../loader/loader.c), split out so that
 * the boot window and the rescue loop share it.
 * Frames (8 bytes, both directions): [cmd][seq 0,1,.. | 0xFF last][len <= 5][5 data bytes] (doc.md).
 * ICRRDY: store a received byte. ICXRDY: load the next byte to send (byte 0 of each reply frame is preloaded
 * into ICDXR). End of a transaction = AAS falling: 8 bytes written = frame received, 8 bytes read = frame
 * delivered. */
#include "link.h"
#include "hw.h"

#define I(off)  REG(I2C0_BASE + (off))
#define OWN     0x29u

static uint8_t  rx[8];
static unsigned rx_n;
static uint8_t  msg[LINK_MAX_MSG];
static unsigned msg_len;
static unsigned msg_expect;     /* next expected chunk, 0xFFFF = none in progress */
static uint8_t  resp[LINK_MAX_RESP];
static unsigned resp_len, resp_pos, resp_seq;
static uint8_t  resp_cmd;
static uint8_t  tx[8];
static unsigned tx_i;
static unsigned in_xfer;

void link_reply(uint8_t cmd, const uint8_t *d, unsigned n)
{
    unsigned i;
    resp_cmd = cmd;
    resp_len = n > LINK_MAX_RESP ? LINK_MAX_RESP : n;
    for (i = 0; i < resp_len; i++)
        resp[i] = d[i];
    resp_pos = 0;
    resp_seq = 0;
}

static void build_tx(void)
{
    unsigned left = resp_len - resp_pos, n = left > 5 ? 5 : left, i;
    tx[0] = resp_cmd;
    tx[1] = left > 5 ? (uint8_t)resp_seq : 0xFF;
    tx[2] = (uint8_t)n;
    for (i = 0; i < 5; i++)
        tx[3 + i] = i < n ? resp[resp_pos + i] : 0;
    I(I2C_DXR) = tx[0];
    tx_i = 1;
}

static void frame_delivered(void)
{
    if (tx[1] == 0xFF) {
        resp_len = 0;
        link_on_reply_read();               /* may jump or reboot */
        link_reply(resp_cmd, 0, 0);
    } else {
        resp_pos += 5;
        resp_seq++;
    }
    build_tx();
}

static void frame_received(void)
{
    unsigned seq = rx[1], n = rx[2] > 5 ? 5 : rx[2], i;
    if (seq == 0 || (seq == 0xFF && msg_expect == 0xFFFF)) {
        msg_len = 0;
        msg_expect = 0;
    } else if (seq != 0xFF && seq != msg_expect) {
        msg_expect = 0xFFFF;                /* unexpected chunk: drop the message */
        return;
    }
    for (i = 0; i < n && msg_len < LINK_MAX_MSG; i++)
        msg[msg_len++] = rx[3 + i];
    if (seq == 0xFF) {
        msg_expect = 0xFFFF;
        link_on_message(rx[0], msg, msg_len);
        build_tx();
    } else {
        msg_expect = seq + 1;
    }
}

void link_init(void)
{
    I(I2C_MDR) = 0;                         /* module reset */
    I(I2C_IMR) = 0;
    I(I2C_OAR) = OWN;
    I(I2C_PSC) = 11;                        /* 10.24 MHz module clock, as the firmware (validated on HW) */
    I(I2C_CLKL) = 5;
    I(I2C_CLKH) = 5;
    I(I2C_EMDR) = 0;
    I(I2C_MDR) = MDR_IRS | MDR_FREE;        /* slave receiver */
    rx_n = 0;
    msg_len = 0;
    msg_expect = 0xFFFF;
    in_xfer = 0;
    link_reply('E', 0, 0);
    build_tx();
}

void link_poll(void)
{
    uint32_t st = I(I2C_STR);
    if (st & STR_RRDY) {
        uint8_t b = (uint8_t)I(I2C_DRR);
        if (rx_n < 8) rx[rx_n] = b;
        rx_n++;
    }
    if (st & STR_XRDY) {
        I(I2C_DXR) = tx_i < 8 ? tx[tx_i] : 0xFF;
        tx_i++;
    }
    if (st & STR_AAS) {
        in_xfer = 1;
    } else if (in_xfer) {
        in_xfer = 0;
        if (rx_n == 8) frame_received();
        else if (tx_i >= 9) frame_delivered();
        rx_n = 0;
    }
}

int link_idle(void)
{
    return !in_xfer && resp_len == 0 && msg_expect == 0xFFFF;
}

int link_bus_free(void)
{
    return !(I(I2C_STR) & STR_BB);
}

void link_shutdown(void)
{
    I(I2C_MDR) = 0;
}
