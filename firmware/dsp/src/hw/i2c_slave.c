/* I2C0 slave at 0x29: link with the TAS1020B, compatible with the stock Bose TAS firmware.
 *
 * Frame (8 bytes, both directions): [cmd][seq][len <= 5][5 data bytes]
 *   seq = 0, 1, 2... for each chunk of a long message, 0xFF for the last (or only) chunk.
 * The TAS writes request frames, and for commands that expect a reply ('S', 'E', 'D', 'd') it waits a few
 * milliseconds then reads reply frames until seq = 0xFF (doc.md, TAS -> DSP I2C protocol section).
 *
 * Byte-level work is done in the INT14 ISR; message handling runs in the main loop (i2c_slave_poll).
 * Messages: P (ALIVE), S (status, extended: include/volume.h), V, m, M, X, U, F, E (console), D/d (refused),
 * R (sampling rate, OpenCompanion TAS only, no reply). Note: X, U, M, m, V are sent at link start-up and on
 * change only; a RAM-loaded firmware gets their stock values from the loader handoff (src/handoff.c). */
#include <string.h>
#include <c6x.h>
#include "regs_i2c.h"
#include "oc.h"
#include "inputs.h"
#include "volume.h"
#include "rate.h"

#define I(off) I2C_REG(I2C0_BASE, off)

#define OWN_ADDR    0x29
#define FRAME_SIZE  8
#define CHUNK       5
#define RX_FRAMES   16                  /* ring of received frames (power of two: index masks) */
#define MSG_MAX     64
#define TX_FRAMES   ((MSG_MAX + CHUNK - 1) / CHUNK)

/* --- ISR side --- */
static volatile uint8_t rx_ring[RX_FRAMES][FRAME_SIZE];
static volatile int rx_head, rx_tail;
static uint8_t rx_cur[FRAME_SIZE];
static int rx_pos;

static volatile uint8_t tx_frames[TX_FRAMES][FRAME_SIZE];
static volatile int tx_count, tx_next;  /* reply frames queued / frame currently offered to the TAS */
static uint8_t tx_cur[FRAME_SIZE];      /* frame being read by the TAS; byte 0 is preloaded into ICDXR */
static int tx_i;                        /* index of the next byte to write into ICDXR */
static uint8_t last_cmd = 'E';

/* --- main loop side --- */
static uint8_t msg[MSG_MAX];
static int msg_len;
static uint8_t msg_cmd;
static char tas_version[16];

/* Offers the current reply frame (or an empty "last" frame) and preloads its byte 0. */
static void load_tx_frame(void)
{
    int k;
    if (tx_next < tx_count) {
        for (k = 0; k < FRAME_SIZE; k++) tx_cur[k] = tx_frames[tx_next][k];
    } else {
        tx_cur[0] = last_cmd;
        tx_cur[1] = 0xFF;
        for (k = 2; k < FRAME_SIZE; k++) tx_cur[k] = 0;
    }
    I(I2C_ICDXR) = tx_cur[0];
    tx_i = 1;
}

void i2c_slave_init(void)
{
    I(I2C_ICMDR) = 0;                     /* module in reset */
    I(I2C_ICOAR) = OWN_ADDR;
    I(I2C_ICPSC) = 11;                    /* SYSCLK2 122.88 MHz / 12 = 10.24 MHz module clock (SPRU175D: 6.7..13.3 MHz) */
    I(I2C_ICCLKL) = 5;
    I(I2C_ICCLKH) = 5;
    I(I2C_ICEMDR) = 0;
    I(I2C_ICIMR) = I2C_ICRRDY | I2C_ICXRDY | I2C_SCD | I2C_AAS_IE;
    I(I2C_ICMDR) = I2C_MDR_IRS | I2C_MDR_FREE;   /* slave receiver, out of reset */
    rx_head = rx_tail = rx_pos = 0;
    tx_count = tx_next = 0;
    load_tx_frame();
    strcpy(tas_version, "?");
}

/* End of an I2C transaction (STOP seen): deliver the received frame, or move on after the TAS read a frame. */
static void end_of_transaction(void)
{
    if (rx_pos == FRAME_SIZE) {
        int next = (rx_head + 1) & (RX_FRAMES - 1);
        if (next != rx_tail) {
            memcpy((void *)rx_ring[rx_head], rx_cur, FRAME_SIZE);
            rx_head = next;
        }
    }
    rx_pos = 0;
    if (tx_i >= FRAME_SIZE + 1) {         /* 8 bytes sent + the request for a 9th: whole frame read */
        if (tx_next < tx_count) tx_next++;
        load_tx_frame();
    } else if (tx_i > 1) {                /* partial read: offer the same frame again */
        load_tx_frame();
    }
}

/* Same scheme as the RAM loader, which was validated on hardware: work from the ICSTR flags (not ICIVR, whose
 * priority order reports XRDY before AAS/SCD), byte 0 of the offered frame always preloaded in ICDXR, each XRDY
 * loads the next byte, and the STOP ends the transaction. */
void i2c_slave_isr(void)
{
    for (;;) {
        uint32_t st = I(I2C_ICSTR);
        int busy = 0;
        if (st & I2C_ICRRDY) {
            uint8_t b = (uint8_t)I(I2C_ICDRR);
            if (rx_pos < FRAME_SIZE) rx_cur[rx_pos] = b;
            rx_pos++;
            busy = 1;
        }
        if (st & I2C_ICXRDY) {
            I(I2C_ICDXR) = tx_i < FRAME_SIZE ? tx_cur[tx_i] : 0xFF;
            tx_i++;
            busy = 1;
        }
        if (st & I2C_SCD) {
            I(I2C_ICSTR) = I2C_SCD;           /* write 1 to clear */
            end_of_transaction();
            busy = 1;
        }
        if (!busy) break;
    }
    (void)I(I2C_ICIVR);                   /* acknowledge any remaining interrupt source */
}

static void reply(uint8_t cmd, const uint8_t *data, int len)
{
    int f, n = 0, frames;
    if (len > MSG_MAX) len = MSG_MAX;
    for (frames = 1; frames * CHUNK < len; frames++)    /* ceil(len / CHUNK), at least 1 (no RTS division) */
        ;
    for (f = 0; f < frames; f++) {
        int chunk = len - n > CHUNK ? CHUNK : len - n;
        volatile uint8_t *fr = tx_frames[f];
        int k;
        fr[0] = cmd;
        fr[1] = (f == frames - 1) ? 0xFF : (uint8_t)f;
        fr[2] = (uint8_t)chunk;
        for (k = 0; k < CHUNK; k++) fr[3 + k] = k < chunk ? data[n + k] : 0;
        n += chunk;
    }
    {
        unsigned csr = _disable_interrupts();
        last_cmd = cmd;
        tx_count = frames;
        tx_next = 0;
        load_tx_frame();
        _restore_interrupts(csr);
    }
}

static void handle_message(uint8_t cmd, const uint8_t *d, int len)
{
    static const uint8_t ALIVE[] = "ALIVE";
    static const uint8_t ERR[] = "ERROR";
    static const uint8_t NAK[] = { 0x15 };
    uint8_t out[MSG_MAX];
    int n;

    switch (cmd) {
    case 'P': reply(cmd, ALIVE, 5); break;
    case 'S': {                           /* status poll, stock 0x10016030: [0x540E standby][0x540F fault];
                                           * either byte != 0 makes the TAS enter standby (PollDspStatus).
                                           * + 3 OpenCompanion bytes (pod, knob, events), same single frame */
        uint8_t st[5];
        st[0] = g_standby.request;
        st[1] = g_standby.fault;
        reply(cmd, st, 2 + volume_status(st + 2));
        break;
    }
    case 'V': if (len >= 2) audio_set_volume_raw((int16_t)((d[0] << 8) | d[1])); break;
    case 'm': if (len >= 1) g_audio.usb_mute = d[0] != 0; break;
    case 'M': if (len >= 1) g_audio.pod_mute = d[0] != 0; break;   /* != 0 = muted (stock 0x10015F1C) */
    case 'X':                             /* stock 0x1001494C (no 99 % rule); 'X' 00 clears the knob validity */
        if (len >= 1) {
            g_audio.pod_present = d[0] != 0;
            if (!d[0]) g_vol.knob_valid = 0;
        }
        break;
    case 'R':                             /* [rate, 24-bit big-endian Hz][layout]; the TAS never reads a reply */
        if (len >= 3) rate_request(((uint32_t)d[0] << 16) | ((uint32_t)d[1] << 8) | d[2]);
        if (len >= 4) {
            layout_request(d[3]);
            g_audio.stream_open = d[3] & 1;   /* the host streams: HOST mode, whatever DSPINT says (volume.c) */
        }
        return;
    case 'U':                             /* USB up: release DSPINT (stock code 7, control.md) */
        g_audio.usb_up = 1;
        mcasp0_dspint_release();
        break;
    case 'F':
        n = len < (int)sizeof(tas_version) - 1 ? len : (int)sizeof(tas_version) - 1;
        memcpy(tas_version, d, n);
        tas_version[n] = 0;
        break;
    case 'E':
        n = console_exec(d, len, out, sizeof(out));
        reply(cmd, out, n);
        break;
    case 'D':
    case 'd': reply(cmd, NAK, 1); break;  /* Bose parameter protocol: not supported */
    default:  reply(cmd, ERR, 5); break;
    }
}

const char *i2c_tas_version(void) { return tas_version; }

int i2c_reply_drained(void) { return tx_next >= tx_count; }

void i2c_slave_poll(void)
{
    while (rx_tail != rx_head) {
        uint8_t fr[FRAME_SIZE];
        int len;
        memcpy(fr, (const void *)rx_ring[rx_tail], FRAME_SIZE);
        rx_tail = (rx_tail + 1) & (RX_FRAMES - 1);

        len = fr[2] > CHUNK ? CHUNK : fr[2];
        if (fr[0] != msg_cmd || fr[1] == 0) {   /* new message (first chunk or other command) */
            msg_len = 0;
            msg_cmd = fr[0];
        }
        if (msg_len + len <= MSG_MAX) {
            memcpy(msg + msg_len, fr + 3, len);
            msg_len += len;
        }
        if (fr[1] == 0xFF) {
            handle_message(msg_cmd, msg, msg_len);
            msg_len = 0;
        }
    }
}
