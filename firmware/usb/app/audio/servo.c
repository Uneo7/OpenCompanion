/*
 * ACG servo, port of the stock AdjustAcgFrequency 0x0813 / ResetAcgTracking 0x14F5 / PSOF handler 0x0A4E.
 * Every SOF: delta = MCLKO1 capture - previous (nominal 24 576 at 48 kHz); acc += nominal - delta; every 4 frames
 * N -= 8 x ((acc - acc_last) + acc / 256), written to ACG1FRQ.
 * DMA re-alignment: if the ISO buffer content is not whole slots, P1.7 = 1 (BADDATA to the DSP) and DMA off;
 * DMACTL0 = 0x01 four frames later, DMA on and P1.7 = 0 from the fifth (stock sequence).
 * Differences with stock: N is clamped to +/-1 % of nominal; N starts at nominal. 48 kHz only since 0.5.1 (user
 * decision 2026-11-02: the host resamples 44.1 kHz content).
 * Level guard (0.5.1): the DMA is also re-aligned when the buffer content latched at SOF exceeds LEVEL_MAX. The
 * UBM writes a packet at up to 1.5 bytes/us while the DMA drains the previous one: with more than ~605 bytes at SOF
 * (5.1, 576-byte packets; 640 in 2.1) the write pointer catches the read pointer inside the 960-byte ring
 * (audio/stream.c sizing rule). The DMA then sees "empty" with the read pointer in the middle of a frame and
 * resumes at the next C-port frame start: every slot shifts (hardware result 2026-11-02 at 44.1 kHz: stereo
 * content wandering across the 6 TDM slots, ISO buffer reading 154..162 = 960 + 154 wrapped, no re-alignment
 * counted). The whole-slot check above cannot see a whole-slot shift. Normal levels: ~576..588 (5.1), ~384 (2.1).
 */
#include <8051.h>

#include "config.h"
#include "servo.h"
#include "stream.h"
#include "tas1020b.h"

#define N_48000      0x61A80FUL         /* 24.576 MHz, DM table 2-3 (stock InitClockRegs: 61 A8 0F) */
#define NOM          24576              /* MCLK cycles per 1 ms frame at 24.576 MHz */
#define NOM_LO       0x4CCD             /* plausible capture deltas: stock bounds 0.8 .. 1.2 x NOM */
#define NOM_HI       0x7333
#define LEVEL_MAX    600                /* bytes in the ring at SOF before the next packet could overrun it */
#define OFF_RANGE    8000               /* x 8 LSB = 0xFA00 ~ 1 % of N */

__data uint8_t servo_resyncs;
__data int16_t servo_off;               /* N - nominal N, in units of 8 LSB */
__data int16_t servo_acc;

static __data int16_t acc_last;
static __data uint16_t cap_prev;
static __data uint8_t frames;
static __data uint8_t resync;           /* 0 = idle, else frames since the DMA was stopped */
static __bit first;

/* No 32-bit arithmetic: |8 x servo_off| < 65536, so the high byte only changes by the carry and the sign. */
static void write_freq(void)
{
    uint16_t lo = (uint16_t)N_48000 + ((uint16_t)servo_off << 3);
    uint8_t hi = (uint8_t)(N_48000 >> 16);
    if (lo < (uint16_t)N_48000) {
        hi++;
    }
    if (servo_off < 0) {
        hi--;
    }
    ACG1FRQ2 = hi;
    ACG1FRQ1 = (uint8_t)(lo >> 8);
    ACG1FRQ0 = (uint8_t)lo;                     /* LSB last: loads the synthesizer (DM 2.2.6.1) */
}

void servo_reset(void)
{
    first = 1;
    frames = 0;
    resync = 0;
    servo_acc = 0;
    acc_last = 0;
    P1_7 = 0;                                   /* stock ResetAcgTracking 0x1512 */
}

void servo_init(void)
{
    servo_off = 0;
    write_freq();
    servo_reset();
}

static void dma_realign(void)
{
    uint8_t r;

    if (resync) {
        resync++;
        if (resync == 5) {
            DMACTL0 = EP_ISO_OUT;               /* stock: 0x01 at the 4th frame */
        } else if (resync > 5) {
            DMACTL0 = DMACTL_DMAEN | EP_ISO_OUT;/* stock: 0x81 from the 5th frame */
            P1_7 = 0;
            resync = 0;
        }
        return;
    }
    /* Whole slots (mask 0 while idle; 256 is a multiple of 4, so the low byte alone decides), and the level guard
     * (top of this file): count > LEVEL_MAX = 0x258. The count never exceeds the 960-byte ring (high byte <= 3). */
    _Static_assert(LEVEL_MAX == 0x258, "level test below");
    r = DMABCNT0L & stream_slot_mask;
    if (stream_slot_mask && (DMABCNT0H > 2 || (DMABCNT0H == 2 && DMABCNT0L > 0x58))) {
        r = 1;
    }
    if (r) {
        P1_7 = 1;
        DMACTL0 = 0x05;                         /* stock value: DMAEN = 0 */
        resync = 1;
        servo_resyncs++;
    }
}

void servo_sof(void)
{
    uint16_t cap, delta;
    uint8_t guard;

    dma_realign();

    cap = ACGCAPL;                              /* stock order: FFE4 then FFE3 */
    cap |= (uint16_t)ACGCAPH << 8;
    frames++;
    if (first) {
        first = 0;
        cap_prev = cap;
        return;
    }
    delta = cap - cap_prev;
    cap_prev = cap;
    for (guard = 0; delta > NOM_HI || delta < NOM_LO; guard++) {
        if (guard == 3) {
            return;                             /* capture not plausible: skip this frame */
        }
        delta -= NOM;                           /* missed SOF: one more frame */
        frames++;
    }
    servo_acc += (int16_t)(NOM - delta);

    if (frames < 4) {
        return;
    }
    frames = 0;
    servo_off -= (servo_acc - acc_last) + (servo_acc >> 8);
    acc_last = servo_acc;
    if (servo_off > OFF_RANGE) {
        servo_off = OFF_RANGE;
    } else if (servo_off < -OFF_RANGE) {
        servo_off = -OFF_RANGE;
    }
    write_freq();
}
