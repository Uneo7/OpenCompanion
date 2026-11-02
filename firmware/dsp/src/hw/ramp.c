/* McASP0 ramp converter (pod volume IVOL, bass knob BEQ+/BEQ-, supply sense VRAWSEN).
 * Port of the stock firmware (re/dsp/out/img.dis), service manual "Conversion Cycle and Ramp Generation".
 *
 * Hardware: McASP0 receives a 6-slot x 32-bit TDM frame at 96 kHz bit clock (2 ms). The frame sync AFSR0
 * (DISCHG) is high during slot 0 and discharges the ramp capacitor C6201; during slots 1..5 the current
 * source Q6201/R6207 charges it. Comparators U6200 compare the ramp with each input; their outputs feed
 * serializers AXR0[0] = BEQ+, AXR0[1] = BEQ-, AXR0[2] = IVOL, AXR0[3] = VRAWSEN. A comparator reads 1 while
 * the ramp is below the input, so the number of leading ones from slot 1 is proportional to the input.
 * Every 50 frames, AXR0[6] is driven high for one frame: it forces VRAWSEN to 3.3 V, and the count read on
 * serializer 3 during that frame becomes the full-scale reference ("ramp calibration").
 *
 * Stock code ported here:
 *   0x10019418  McASP0 init: RGBLCTL = 0, RMASK = 0xFFFFFFFF, RFMT = 0x80F0, AFSRCTL = 0x312 (6-slot TDM,
 *               slot-wide internal FS), ACLKRCTL = 0xA0, AHCLKRCTL = 0x80FF (internal, AUXCLK / 256 = 96 kHz),
 *               RTDM = 0x3F, RINTCTL = 0x80 (RSTAFRM), RCLKCHK = 0, SRCTL0..3 = RX, PFUNC = 0xFFF0,
 *               PDIR = 0x80000400; then RHCLKRST, RCLKRST, dMAX setup, RSRCLR, RSMRST, RFRST (each polled),
 *               then IER |= INT9.
 *   dMAX (same function): LoMAX event 5 (McASP0 RX) = 0x48523303: 4 elements per quantum, TCC 8 + interrupt,
 *               reload (ping-pong), 32-bit, transfer entry PTE 0x33 (0x620080CC): src = McASP0 data port,
 *               counts 0x604 (COUNT1 = 6 slots, COUNT0 = 4 serializers), DSTINDX0 = 7, SRCINDX0 = 1,
 *               DSTINDX1 = -20, SRCINDX1 = -3 -> buffer[serializer][7] with slot = 0..5; pong = ping + 0x70.
 *               LoMAX event 26 (McASP0 error/interrupt) = 0x00020007: CPU interrupt on INT9.
 *               RSTAT = XSTAT = 0xFFFF, DELPR |= bits 5/26, DEPR |= bits 5/26, DEER = bit 26 then bit 5.
 *   0x1001A360  INT9 (RSTAFRM, start of each McASP0 frame): float frame counter 0x10013790 += 1; at 50:
 *               PDSET = 0x40, PDIR |= 0x40 (AXR0[6] drives VRAWSEN high), counter = 0. Writes RSTAT back.
 *   0x1001A3E0  INT8, TCC 8 (one frame received): ping-pong index 0x100137B6 ^= 1; if PDIR bit 6 is set, the
 *               frame just received is a calibration frame: PDIR &= ~0x40 and flag it; post the converter task.
 *   0x10019F8C  object init (0x100136B0): index = 1, cal = 100.0, everything else 0.
 *   0x100162B4  per-serializer conversion (count, calibration window, clamp, ratio).
 *   0x10016420  frame conversion: VRAW, IVOL (smoothing + hysteresis -> pod %), BEQ (bass value).
 * Differences: the stock converter runs in a DSP/BIOS task (priority 3) posted by INT8; here ramp_poll()
 * runs in the main loop. The calibration flag is kept in an array instead of word 20 of each buffer. The
 * stock console overrides (VOM 0x100137B4, BTO 0x100137B5) do not exist in OpenCompanion. */
#include <c6x.h>
#include "oc.h"
#include "ramp.h"
#include "volume_curve.h"
#include "regs_mcasp.h"
#include "regs_dmax.h"

#define M0(off)        MCASP_REG(MCASP0_BASE, off)
#define CAL_BIT        (1u << 6)            /* AXR0[6]: ramp calibration output */
#define CAL_PERIOD     50.0f                /* frames between calibrations (stock 50.0f) */

#define RAMP_SER       4                    /* AXR0[0..3] */
#define RAMP_STRIDE    7                    /* stock buffer layout: 7 words per serializer, slots 0..5 used */
#define SER_BEQ_P      0
#define SER_BEQ_N      1
#define SER_IVOL       2
#define SER_VRAW       3

#define DMAX_EVT_MCASP0_RX   5
#define DMAX_EVT_MCASP0_INT  26
#define RAMP_PTE             0x33
#define RAMP_TCC             8              /* DTCR1 bit 0 */
#define LOMAX_EVENT(n)       REG32(LOMAX_PARAM + 4u * (n))

typedef uint32_t ramp_buf_t[RAMP_SER * RAMP_STRIDE];

#pragma DATA_ALIGN(bufs, 8)
static ramp_buf_t bufs[2];
static volatile int cal_flag[2];
static volatile int buf_index = 1;          /* stock 0x100137B6, init 1 (0x10019F8C) */
static volatile int frame_ready;            /* posted by the TCC 8 interrupt */
static float frame_counter;                 /* stock 0x10013790 (float) */

ramp_state_t g_ramp;

/* Bounded "set bit and wait for read-back", same steps as the stock (which waits forever). */
static void rgbl_set(uint32_t bit)
{
    unsigned k;
    M0(MCASP_RGBLCTL) = M0(MCASP_RGBLCTL) | bit;
    for (k = 0; k < 200000u; k++)
        if (M0(MCASP_RGBLCTL) & bit) return;
}

static void dmax_setup(void)
{
    volatile dmax_xfer_t *x = (volatile dmax_xfer_t *)(LOMAX_PARAM + 4u * RAMP_PTE);
    const uint32_t ev = (1u << DMAX_EVT_MCASP0_RX) | (1u << DMAX_EVT_MCASP0_INT);

    DMAX_DEDR = ev;
    LOMAX_EVENT(DMAX_EVT_MCASP0_RX) = 0x48523303u;   /* = EV_QTSL(1)|EV_TCC(8)|EV_TCINT|EV_RLOAD|ESIZE 32|PTE 0x33|GP */
    x->src         = MCASP0_DATA;
    x->dst         = (uint32_t)bufs[0];
    x->counts      = DMAX_COUNTS(0, 6, RAMP_SER);    /* 0x00000604 */
    x->idx0        = DMAX_IDX(RAMP_STRIDE, 1);       /* 0x00070001 */
    x->idx1        = DMAX_IDX(-20, -3);              /* 0xFFECFFFD */
    x->idx2        = DMAX_IDX(1, 1);                 /* 0x00010001 */
    x->ref_counts  = DMAX_COUNTS(0, 6, RAMP_SER);
    x->src_reload0 = MCASP0_DATA;
    x->dst_reload0 = (uint32_t)bufs[0];
    x->src_reload1 = MCASP0_DATA;
    x->dst_reload1 = (uint32_t)bufs[1];
    LOMAX_EVENT(DMAX_EVT_MCASP0_INT) = 0x00020007u;  /* interrupt entry: INT9 */

    M0(MCASP_RSTAT) = 0xFFFFu;
    M0(MCASP_XSTAT) = 0xFFFFu;
    DMAX_DELPR = ev;                                 /* LoMAX */
    DMAX_DEPR |= ev;                                 /* rising edge */
    DMAX_DTCR1 = 1u << (RAMP_TCC - 8);
    /* Not in stock (cold reset): drop events latched while the stock firmware was stopped by the RAM loader,
     * same reason as the McASP1 fix in dmax.c. */
    DMAX_DEFR = ev;
    DMAX_DEER = 1u << DMAX_EVT_MCASP0_INT;
    DMAX_DEER = 1u << DMAX_EVT_MCASP0_RX;
}

void ramp_init(void)
{
    unsigned csr;
    g_ramp.cal = 100.0f;                    /* 0x10019F8C */
    g_ramp.vraw = 12.0f;                    /* initial value of 0x10010148 */
    buf_index = 1;
    frame_counter = 0.0f;
    frame_ready = 0;

    M0(MCASP_RGBLCTL)   = 0;                /* receive section in reset (stock) */
    M0(MCASP_RMASK)     = 0xFFFFFFFFu;
    M0(MCASP_RFMT)      = 0x000080F0u;      /* MSB first, 32-bit slots, no delay, DMA port */
    M0(MCASP_AFSRCTL)   = 0x00000312u;      /* 6-slot TDM, slot-wide FS (DISCHG), internal, active high */
    M0(MCASP_ACLKRCTL)  = 0x000000A0u;      /* internal bit clock, divide by 1 */
    M0(MCASP_AHCLKRCTL) = 0x000080FFu;      /* internal high clock, divide by 256 */
    M0(MCASP_RTDM)      = 0x0000003Fu;      /* slots 0..5 */
    M0(MCASP_RINTCTL)   = 0x00000080u;      /* start-of-frame interrupt (-> dMAX event 26 -> INT9) */
    M0(MCASP_RCLKCHK)   = 0;
    M0(MCASP_SRCTL(0))  = SRMOD_RX;
    M0(MCASP_SRCTL(1))  = SRMOD_RX;
    M0(MCASP_SRCTL(2))  = SRMOD_RX;
    M0(MCASP_SRCTL(3))  = SRMOD_RX;
    M0(MCASP_PFUNC)     = 0x0000FFF0u;
    /* Stock writes PDIR = 0x80000400 here, before main sets DSPINT (bit 9) and before the frame task can
     * drive AXR0[14] (bit 14, spk_mode.c): keep both states. */
    csr = _disable_interrupts();
    M0(MCASP_PDIR) = 0x80000400u | (M0(MCASP_PDIR) & ((1u << 9) | (1u << 14)));
    _restore_interrupts(csr);

    rgbl_set(GBL_RHCLKRST);
    rgbl_set(GBL_RCLKRST);
    dmax_setup();
    rgbl_set(GBL_RSRCLR);
    rgbl_set(GBL_RSMRST);
    rgbl_set(GBL_RFRST);
    ICR = 1u << 9;
    IER |= 1u << 9;
}

/* INT9: start of a McASP0 frame (stock 0x1001A360). */
void ramp_on_frame_start(void)
{
    uint32_t st = M0(MCASP_RSTAT);
    if (st & (1u << 6)) {                   /* RSTAFRM */
        frame_counter += 1.0f;
        if (frame_counter == CAL_PERIOD) {
            M0(MCASP_PDIN) = CAL_BIT;       /* PDSET */
            frame_counter = 0.0f;
            M0(MCASP_PDIR) |= CAL_BIT;
        }
    }
    M0(MCASP_RSTAT) = st;
}

/* INT8 handler: TCC 8 (stock 0x1001A484..0x1001A524). */
void ramp_on_dma_irq(void)
{
    int idx;
    if (!(DMAX_DTCR1 & 1u)) return;
    DMAX_DTCR1 = 1u;
    idx = buf_index ^ 1;
    buf_index = idx;
    if (M0(MCASP_PDIR) & CAL_BIT) {
        M0(MCASP_PDIR) &= ~CAL_BIT;
        cal_flag[idx] = 1;
    } else {
        cal_flag[idx] = 0;
    }
    frame_ready = 1;
}

/* Leading ones from slot 1, continuing into the next slots while the first edge is not found
 * (stock loop 0x10016308..0x10016358, 5 slots at most). */
static uint32_t count_ones(const uint32_t *ser)
{
    uint32_t n = 0;
    int left = 5;
    const uint32_t *p = ser + 1;
    for (;;) {
        n += _lmbd(0, *p++);
        if (n < 32u) break;
        if (--left == 0) break;
    }
    return n;
}

/* Stock 0x100162B4. kind: 0 = calibration, 1 = VRAW (not clamped), 2 = other (clamped to full scale). */
static void measure(const uint32_t *buf, int ser, float *dst, int kind)
{
    uint32_t n = count_ones(buf + ser * RAMP_STRIDE);
    float c;
    g_ramp.count[ser] = n;
    if (kind == 0) {
        g_ramp.cal = (n > 120u || n < 80u) ? 100.0f : (float)n;
        return;
    }
    c = (float)n;
    if (kind == 2 && c > g_ramp.cal)
        c = (float)(int32_t)g_ramp.cal;          /* 80..120: same truncation as stock, no RTS call */
    *dst = c / g_ramp.cal;
}

/* Stock 0x10016420..0x100165EC: pod knob. */
static uint32_t ivol_last;
static int ivol_score = 256;             /* invalid until the reading has been plausible for 0.5 s */

static void convert_ivol(const uint32_t *buf)
{
    measure(buf, SER_IVOL, &g_ramp.ivol_ratio, 2);
    g_ramp.ivol_ok = (uint8_t)ivol_plausible(g_ramp.count[SER_IVOL], g_ramp.cal, &ivol_last, &ivol_score);
    /* Stock 0x100162B4 / 0x10016420 (volume_curve.c pod_filter); the result is the pod volume 0x100100FC,
     * applied by volume.c in both modes (stock 0x10016638..0x10016680 copies it when a pod is present). */
    g_ramp.pod_pct = pod_filter(g_ramp.ivol_ratio, &g_ramp.ivol_smooth, g_ramp.pod_pct);
}

/* Stock 0x10016690..0x100167B0: bass knob. */
static void convert_beq(const uint32_t *buf)
{
    float p, n, prev, r;
    measure(buf, SER_BEQ_P, &g_ramp.beq_p, 2);
    measure(buf, SER_BEQ_N, &g_ramp.beq_n, 2);
    p = g_ramp.beq_p;
    n = g_ramp.beq_n;
    if (n == 0.0f || p == 0.0f) return;
    prev = g_ramp.bass_ratio;
    g_ramp.bass_ratio = (n - p) / (p + n);
    r = g_ramp.bass_ratio;
    if (r == prev) return;
    if ((double)r < -1.0) g_ramp.bass = 1.0f;
    else if ((double)r > 1.0) g_ramp.bass = -1.0f;
    else g_ramp.bass = (float)((double)r * -1.6666666666666667);
}

void ramp_poll(void)
{
    const uint32_t *buf;
    int idx;
    if (!frame_ready) return;
    frame_ready = 0;
    idx = buf_index;
    buf = bufs[idx];
    if (cal_flag[idx]) {
        measure(buf, SER_VRAW, &g_ramp.cal, 0);
        g_ramp.cal_frames++;
    } else {
        measure(buf, SER_VRAW, &g_ramp.vraw_ratio, 1);
        g_ramp.vraw = (float)(5.1 + (double)g_ramp.vraw_ratio * (3.3 / 0.43));  /* stock: (r x 3.3) / 0.43 */
    }
    convert_ivol(buf);
    convert_beq(buf);
    g_ramp.frames++;
}
