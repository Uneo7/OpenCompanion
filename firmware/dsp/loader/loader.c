/* Started in place of a stock graph module's process function. Stops audio (McASP, dMAX), takes over I2C0 as
 * slave 0x29 WITHOUT dMAX or interrupts. Frames and 'E' protocol: ../README.md, RAM loader. */
#include <stdint.h>

#include "../include/reboot_seq.h"
#include "../include/handoff.h"

#define REG(a)          (*(volatile uint32_t *)(a))

/* I2C0 (SPRU175D) */
#define I2C0            0x49000000u
#define I2C_OAR         REG(I2C0 + 0x00)
#define I2C_IER         REG(I2C0 + 0x04)
#define I2C_STR         REG(I2C0 + 0x08)
#define I2C_DRR         REG(I2C0 + 0x18)
#define I2C_DXR         REG(I2C0 + 0x20)
#define I2C_MDR         REG(I2C0 + 0x24)
#define I2C_PSC         REG(I2C0 + 0x30)
#define STR_ICRRDY      (1u << 3)
#define STR_ICXRDY      (1u << 4)
#define STR_AAS         (1u << 9)
#define MDR_IRS         (1u << 5)
#define TAS_SLAVE_ADDR  0x29u

/* dMAX (SPRU795D) and McASP (SPRU041J) */
#define DMAX_DEDR       REG(0x60000010u)
#define DMAX_DTCR0      REG(0x60000080u)
#define DMAX_DTCR1      REG(0x600000A0u)
#define MCASP_GBLCTL(b) REG((b) + 0x44u)

/* Program cache: writing 0x80000000 to L1PICR invalidates the whole L1P (as the Bose boot driver does). */
#define L1PICR          REG(0x20000004u)

/* DA705 RAM and the area reserved for the loader (never overwritten). */
#define RAM_LO          0x10000000u
#define RAM_HI          0x10030000u
#define STUB_LO         0x10023000u
#define STUB_HI         0x10024E20u     /* + handoff record 0x10024E00..0x10024E1F (handoff.h) */

#define LOADER_VERSION  1
#define ACK             0x06
#define NAK             0x15
#define MAX_MSG         64      /* the TAS relays at most 61 bytes */
#define MAX_RESP        61      /* above this the TAS answers "ERROR4" */

enum { LD_INFO = 0x00, LD_WRITE = 0x01, LD_CRC = 0x02, LD_JUMP = 0x03, LD_READ = 0x04, LD_FILL = 0x05,
       LD_REBOOT = 0x06 };

static uint8_t  rx[8];
static unsigned rx_n;

static uint8_t  msg[MAX_MSG];
static unsigned msg_len;
static unsigned msg_expect;     /* next expected chunk number, 0xFFFF = no message in progress */

static uint8_t  resp[MAX_RESP];
static unsigned resp_len;
static uint8_t  resp_cmd;
static unsigned resp_pos;       /* bytes of resp already placed in delivered frames */
static unsigned resp_seq;

static uint8_t  tx[8];
static unsigned tx_i;           /* index of the next tx byte to write into DXR */

static uint32_t jump_to;        /* pending jump address (0 = none) */
static int reboot_pending;      /* LD_REBOOT acknowledged: reboot once the reply has been read */

static uint32_t rd32(const uint8_t *p)
{
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

static int in_range(uint32_t a, uint32_t n, uint32_t lo, uint32_t hi)
{
    return a >= lo && n <= hi - lo && a - lo <= (hi - lo) - n;
}

static int overlaps_stub(uint32_t a, uint32_t n)
{
    return n && a < STUB_HI && a + n > STUB_LO;
}

/* CRC-32 IEEE (same as zlib.crc32), nibble-wise. */
static uint32_t crc32(const volatile uint8_t *p, uint32_t n)
{
    static const uint32_t t[16] = {
        0x00000000, 0x1DB71064, 0x3B6E20C8, 0x26D930AC, 0x76DC4190, 0x6B6B51F4, 0x4DB26158, 0x5005713C,
        0xEDB88320, 0xF00F9344, 0xD6D6A3E8, 0xCB61B38C, 0x9B64C2B0, 0x86D3D2D4, 0xA00AE278, 0xBDBDF21C };
    uint32_t c = 0xFFFFFFFFu;
    while (n--) {
        c ^= *p++;
        c = (c >> 4) ^ t[c & 15];
        c = (c >> 4) ^ t[c & 15];
    }
    return ~c;
}

static void set_resp(uint8_t cmd, const uint8_t *d, unsigned n)
{
    unsigned i;
    resp_cmd = cmd;
    resp_len = n > MAX_RESP ? MAX_RESP : n;
    for (i = 0; i < resp_len; i++)
        resp[i] = d[i];
    resp_pos = 0;
    resp_seq = 0;
}

/* Build the current response frame and preload its byte 0 into DXR. */
static void build_tx(void)
{
    unsigned left = resp_len - resp_pos, n = left > 5 ? 5 : left, i;
    tx[0] = resp_cmd;
    tx[1] = (left > 5) ? (uint8_t)resp_seq : 0xFF;
    tx[2] = (uint8_t)n;
    for (i = 0; i < 5; i++)
        tx[3 + i] = i < n ? resp[resp_pos + i] : 0;
    I2C_DXR = tx[0];
    tx_i = 1;
}

/* The TAS has read a whole frame: move to the next one (or fall back to an empty response). */
static void frame_delivered(void)
{
    if (tx[1] == 0xFF) {
        if (reboot_pending)
            reboot_to_rom();                     /* does not return */
        if (jump_to) {
            void (*entry)(void) = (void (*)(void))jump_to;
            volatile int k;
            L1PICR = 0x80000000u;               /* invalidate the whole program cache */
            for (k = 0; k < 1000; k++) ;
            entry();                             /* does not return */
        }
        set_resp(resp_cmd, 0, 0);
    } else {
        resp_pos += 5;
        resp_seq++;
    }
    build_tx();
}

static void ack(uint8_t cmd)
{
    uint8_t a = ACK;
    set_resp(cmd, &a, 1);
}

static void nak(uint8_t cmd)
{
    uint8_t a = NAK;
    set_resp(cmd, &a, 1);
}

static void loader_cmd(void)
{
    const uint8_t cmd = 'E';
    uint8_t out[MAX_RESP];
    uint32_t a, n, i;
    if (msg_len < 1) { nak(cmd); return; }
    switch (msg[0]) {
    case LD_INFO:
        out[0] = ACK; out[1] = 'O'; out[2] = 'C'; out[3] = 'L'; out[4] = LOADER_VERSION;
        set_resp(cmd, out, 5);
        return;
    case LD_WRITE:                              /* [01][address][data...] */
        if (msg_len < 5) break;
        a = rd32(msg + 1); n = msg_len - 5;
        if (!in_range(a, n, RAM_LO, RAM_HI) || overlaps_stub(a, n)) break;
        for (i = 0; i < n; i++)
            ((volatile uint8_t *)a)[i] = msg[5 + i];
        ack(cmd);
        return;
    case LD_FILL:                               /* [05][address][length][byte] */
        if (msg_len < 10) break;
        a = rd32(msg + 1); n = rd32(msg + 5);
        if (!in_range(a, n, RAM_LO, RAM_HI) || overlaps_stub(a, n)) break;
        for (i = 0; i < n; i++)
            ((volatile uint8_t *)a)[i] = msg[9];
        ack(cmd);
        return;
    case LD_CRC:                                /* [02][address][length] -> [06][crc32 LE] */
        if (msg_len < 9) break;
        a = rd32(msg + 1); n = rd32(msg + 5);
        if (!in_range(a, n, RAM_LO, RAM_HI) || n > 0x10000u) break;
        i = crc32((const volatile uint8_t *)a, n);
        out[0] = ACK;
        out[1] = (uint8_t)i; out[2] = (uint8_t)(i >> 8); out[3] = (uint8_t)(i >> 16); out[4] = (uint8_t)(i >> 24);
        set_resp(cmd, out, 5);
        return;
    case LD_READ:                               /* [04][address][n] -> [06][n bytes], n <= 60 */
        if (msg_len < 6) break;
        a = rd32(msg + 1); n = msg[5];
        if (n > MAX_RESP - 1 || !in_range(a, n, RAM_LO, RAM_HI)) break;
        out[0] = ACK;
        for (i = 0; i < n; i++)
            out[1 + i] = ((const volatile uint8_t *)a)[i];
        set_resp(cmd, out, n + 1);
        return;
    case LD_REBOOT:                             /* [06] -> [06], reboot once the reply has been read */
        reboot_pending = 1;
        ack(cmd);
        return;
    case LD_JUMP:                               /* [03][address] -> [06], jump once the reply has been read */
        if (msg_len < 5) break;
        a = rd32(msg + 1);
        if (!in_range(a, 4, RAM_LO, RAM_HI) || overlaps_stub(a, 4)) break;
        jump_to = a;
        ack(cmd);
        return;
    }
    nak(cmd);
}

static void message(uint8_t cmd)
{
    static const uint8_t alive[5] = { 'A', 'L', 'I', 'V', 'E' };
    static const uint8_t status[2] = { 0, 0 };   /* byte 1 = 0: no standby request */
    switch (cmd) {
    case 'E': loader_cmd(); break;
    case 'P': set_resp(cmd, alive, 5); break;
    case 'S': set_resp(cmd, status, 2); break;
    default:  set_resp(cmd, 0, 0); break;      /* V, m, M, X, U, F, D, d...: accepted and ignored */
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
        msg_expect = 0xFFFF;                    /* unexpected chunk: drop the message */
        return;
    }
    for (i = 0; i < n && msg_len < MAX_MSG; i++)
        msg[msg_len++] = rx[3 + i];
    if (seq == 0xFF) {
        msg_expect = 0xFFFF;
        message(rx[0]);
    } else {
        msg_expect = seq + 1;
    }
}

static void stop_audio(void)
{
    DMAX_DEDR = 0xFFFFFFFFu;                    /* no more dMAX transfers into RAM */
    DMAX_DTCR0 = 0xFFu;
    DMAX_DTCR1 = 0xFFu;
    MCASP_GBLCTL(0x44000000u) = 0;              /* McASP0: ramp ADC */
    MCASP_GBLCTL(0x45000000u) = 0;              /* McASP1: TDM audio */
    MCASP_GBLCTL(0x46000000u) = 0;              /* McASP2 */
}

static void i2c_slave_init(void)
{
    uint32_t psc = I2C_PSC;                     /* keep the prescaler set by the Bose firmware */
    I2C_MDR = 0;                                /* module reset (IRS = 0) */
    I2C_IER = 0;
    I2C_OAR = TAS_SLAVE_ADDR;
    I2C_PSC = psc;
    I2C_MDR = MDR_IRS;                          /* slave, 7-bit address, 8-bit data */
}

void loader_main(void)
{
    unsigned in_xfer = 0;
    /* .bss is not zero-initialised (no C runtime): reset every state variable here. */
    rx_n = 0;
    msg_len = 0;
    handoff_capture();                          /* stock TAS link state, before anything is overwritten */
    stop_audio();
    i2c_slave_init();
    msg_expect = 0xFFFF;
    jump_to = 0;
    reboot_pending = 0;
    set_resp('E', 0, 0);
    build_tx();

    for (;;) {
        uint32_t st = I2C_STR;
        if (st & STR_ICRRDY) {                  /* byte received (TAS writing) */
            uint8_t b = (uint8_t)I2C_DRR;
            if (rx_n < 8)
                rx[rx_n] = b;
            rx_n++;
        }
        if (st & STR_ICXRDY) {                  /* DXR moved to the shift register (TAS reading) */
            I2C_DXR = tx_i < 8 ? tx[tx_i] : 0xFF;
            tx_i++;
        }
        if (st & STR_AAS) {
            in_xfer = 1;
        } else if (in_xfer) {                   /* end of transaction (STOP or new START) */
            in_xfer = 0;
            if (rx_n == 8)
                frame_received();
            else if (tx_i >= 9)
                frame_delivered();
            rx_n = 0;
        }
    }
}
