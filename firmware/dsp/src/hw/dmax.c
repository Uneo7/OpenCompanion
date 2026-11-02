/* dMAX: McASP1 <-> RAM ping-pong transfers, copied from the Bose configuration
 * (hardware.md section 5, code 0x10018D8C..).
 *
 * Receive (event 7, TCC 5): McASP1 data port -> [serializer][slot][sample]
 *   counters: COUNT0 = 2 serializers, COUNT1 = 8 slots, COUNT2 = 96 samples
 *   indexes (in 32-bit elements): DST0 = +768 (next serializer), DST1 = -672, DST2 = -1439.
 * Transmit (event 6, TCC 2): [slot][sample] -> McASP1 data port
 *   COUNT0 = 1, COUNT1 = 8 slots, COUNT2 = 96; SRC1 = +96, SRC2 = -671.
 * Reload (RLOAD) alternates automatically between address sets 0 and 1 (ping-pong). */
#include "regs_dmax.h"
#include "oc.h"

/* Transfer entry locations in the HiMAX PaRAM (PTE, in words), same as Bose */
#define PTE_RX 0x6A
#define PTE_TX 0x5F
#define TCC_RX 5
#define TCC_TX 2

static volatile dmax_xfer_t *const xfer_rx = (volatile dmax_xfer_t *)(HIMAX_PARAM + 4 * PTE_RX);
static volatile dmax_xfer_t *const xfer_tx = (volatile dmax_xfer_t *)(HIMAX_PARAM + 4 * PTE_TX);
static volatile uint32_t *const event_table = (volatile uint32_t *)HIMAX_PARAM;

/* dMAX bring-up, same as the stock BIOS init routine 0x1001D480 (run at every stock boot, also after a DSP-only
 * reset): stop both MAX cores (0x60000030 = 0, 0x60000000 = 0; registers not in SPRU795D), release CSPRST,
 * load the MAX0 / MAX1 program memories (0x61000000 / 0x62000000, src/hw/dmax_ucode.c), clear both PaRAMs
 * (0x400 words each), start the cores (0x60000000 = 1, 0x60000030 = 3; measured stock values).
 * Without it the dMAX never serves an event after a reset: McASP1 overruns (RSTAT 0x17D), no TCC interrupt, no
 * audio frame, no ramp converter. A RAM-loaded firmware got away without it because the stock firmware had
 * already done it. Must run before any PaRAM write (dmax_audio_init, ramp_init). */
#define DMAX_CORE0   REG32(DMAX_BASE + 0x00)
#define DMAX_CORE1   REG32(DMAX_BASE + 0x30)
#define CFGBRIDGE    REG32(0x40000024u)
#define HIMAX_PROG   ((volatile uint32_t *)0x61000000u)
#define LOMAX_PROG   ((volatile uint32_t *)0x62000000u)
#define PARAM_WORDS  0x400u

extern const uint32_t dmax_ucode_len;
extern const uint32_t dmax_ucode_hi[];
extern const uint32_t dmax_ucode_lo_patch_len;
extern const uint32_t dmax_ucode_lo_patch[][2];

void dmax_hw_init(void)
{
    volatile uint32_t *hp = (volatile uint32_t *)HIMAX_PARAM, *lp = (volatile uint32_t *)LOMAX_PARAM;
    uint32_t i;

    DMAX_DEDR = 0xFFFFFFFFu;            /* no event may start a transfer while the program is replaced */
    DMAX_CORE1 = 0;
    DMAX_CORE0 = 0;
    CFGBRIDGE &= ~1u;
    for (i = 0; i < dmax_ucode_len; i++) {
        HIMAX_PROG[i] = dmax_ucode_hi[i];
        LOMAX_PROG[i] = dmax_ucode_hi[i];
    }
    for (i = 0; i < dmax_ucode_lo_patch_len; i++)
        LOMAX_PROG[dmax_ucode_lo_patch[i][0]] = dmax_ucode_lo_patch[i][1];
    for (i = 0; i < PARAM_WORDS; i++) {
        hp[i] = 0;
        lp[i] = 0;
    }
    CFGBRIDGE &= ~1u;
    DMAX_DTCR0 = 0xFFu;                 /* no stale transfer complete code */
    DMAX_DTCR1 = 0xFFu;
    DMAX_CORE0 = 1;
    DMAX_CORE1 = 3;
}

void dmax_audio_init(int32_t *rx_ping, int32_t *rx_pong, int32_t *tx_ping, int32_t *tx_pong)
{
    const uint32_t rx_counts = DMAX_COUNTS(FRAME_LEN, TDM_SLOTS, RX_SERIALIZERS);
    const uint32_t tx_counts = DMAX_COUNTS(FRAME_LEN, TDM_SLOTS, 1);

    DMAX_DEDR = (1u << DMAX_EVT_MCASP1_RX) | (1u << DMAX_EVT_MCASP1_TX);

    xfer_rx->src         = MCASP1_DATA;
    xfer_rx->dst         = (uint32_t)rx_ping;
    xfer_rx->counts      = rx_counts;
    xfer_rx->idx0        = DMAX_IDX(768, 1);
    xfer_rx->idx1        = DMAX_IDX(-672, -1);
    xfer_rx->idx2        = DMAX_IDX(-1439, -1);
    xfer_rx->ref_counts  = rx_counts;
    xfer_rx->src_reload0 = MCASP1_DATA;
    xfer_rx->dst_reload0 = (uint32_t)rx_ping;
    xfer_rx->src_reload1 = MCASP1_DATA;
    xfer_rx->dst_reload1 = (uint32_t)rx_pong;

    xfer_tx->src         = (uint32_t)tx_ping;
    xfer_tx->dst         = MCASP1_DATA;
    xfer_tx->counts      = tx_counts;
    xfer_tx->idx0        = 0;
    xfer_tx->idx1        = DMAX_IDX(0, FRAME_LEN);
    xfer_tx->idx2        = DMAX_IDX(0, -(TDM_SLOTS - 1) * FRAME_LEN + 1);
    xfer_tx->ref_counts  = tx_counts;
    xfer_tx->src_reload0 = (uint32_t)tx_ping;
    xfer_tx->dst_reload0 = MCASP1_DATA;
    xfer_tx->src_reload1 = (uint32_t)tx_pong;
    xfer_tx->dst_reload1 = MCASP1_DATA;

    /* Event entries: identical to Bose (0x45526A03 at 0x10018DB8, 0x02525F03 at 0x10018E2C) */
    event_table[DMAX_EVT_MCASP1_RX] = EV_QTSL(1) | EV_TCC(TCC_RX) | EV_TCINT | EV_RLOAD
                                    | EV_CC(0) | EV_ESIZE_32 | EV_PTE(PTE_RX) | EV_ETYPE_GP;
    event_table[DMAX_EVT_MCASP1_TX] = EV_QTSL(0) | EV_TCC(TCC_TX) | EV_TCINT | EV_RLOAD
                                    | EV_CC(0) | EV_ESIZE_32 | EV_PTE(PTE_TX) | EV_ETYPE_GP;

    DMAX_DTCR0 = (1u << TCC_RX) | (1u << TCC_TX);   /* clear pending codes */
}

/* Same as Bose 0x10018EA4..0x10018EF8: HiMAX priority, rising-edge polarity, enable TX (6) and RX (7),
 * plus one step Bose does not need: clear the event flags 6/7 in DEFR right before enabling.
 * DEFR latches McASP1 events even while they are disabled, and a latched event is serviced as soon as DEER
 * enables it (SPRU795D 1.x "Event-triggered dMAX Transfer", 2.3.4). Bose starts from a cold reset (DEFR = 0).
 * We start after the stock firmware ran: the RAM loader disables the events (DEDR) before stopping McASP1,
 * so EF6/EF7 stay latched. Without this clear, DEER fires one stale RX transfer (2 elements -> slot 0 of both
 * serializers) and one stale TX transfer while McASP1 is still in reset: every received slot then lands one
 * index late (USB L in index 1, R in 2) and the transmit stream is one slot early. McASP1 is in reset with
 * DMA requests off here (mcasp1_init), so no new edge can be latched before mcasp1_start(). */
void dmax_enable_audio(void)
{
    const uint32_t ev = (1u << DMAX_EVT_MCASP1_RX) | (1u << DMAX_EVT_MCASP1_TX);
    DMAX_DEHPR = ev;     /* handled by HiMAX */
    DMAX_DEPR |= ev;     /* rising edge (Bose value) */
    DMAX_DEFR  = ev;     /* drop stale latched events (write 1 to clear), after the polarity write */
    DMAX_DEER  = ev;
}

static uint32_t irq_count;

/* Called from the INT8 ISR: returns the pending transfer complete codes and acknowledges them
 * (SPRU795D 2.3.12: TCC pending bits are cleared only by writing 1 to DTCR0). */
uint32_t dmax_take_tcc(void)
{
    uint32_t pending = DMAX_DTCR0;
    DMAX_DTCR0 = pending;
    irq_count++;
    return pending;
}
