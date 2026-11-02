/* Audio buffers and frame timing: received frame k (ping or pong) is processed into the transmit buffer
 * of the same parity, like the Bose firmware (ping-pong index toggled at each receive completion).
 * TODO (hardware): check RX/TX alignment (one frame of latency) and that the CPU never touches a buffer
 * the dMAX is currently using. */
#include "oc.h"
#include "prof.h"

/* CPU load, measured (no cycle counter on the C67x+): RTI counter 1 (free-running counter RTIFRC1, set up as
 * the stock DSP/BIOS does: RTICPUC1 = 1, GCTRL bit 1; stock 0x1002BF30..0x1002BFE8; counter 0 is left alone).
 * Frame period = ticks between two receive completions (ISR); busy = ticks spent in audio_process(). Load =
 * busy / period, independent of the RTI clock. g_cpu: average (1/16 per frame), maximum since the last CPU
 * read, frames lost (a frame completed while the previous one was still waiting). */
#define RTI(off)     (*(volatile uint32_t *)(0x42000000u + (off)))
#define RTIGCTRL     0x00u
#define RTIFRC1      0x30u
#define RTIUC1       0x34u
#define RTICPUC1     0x38u

cpu_load_t g_cpu;
#ifdef OC_PROF
uint32_t g_prof[PROF_N];
volatile uint32_t g_prof_t, g_prof_isr_acc;
#endif
static uint32_t isr_t;

#pragma DATA_ALIGN(rx_bufs, 8)
#pragma DATA_ALIGN(tx_bufs, 8)
static rx_buf_t rx_bufs[2];
static tx_buf_t tx_bufs[2];

static volatile int frame_ready;  /* 0 = none; 1 = ping ready; 2 = pong ready */
static int rx_index;              /* next receive buffer to complete */

void audio_io_init(void)
{
    int i, s, n;
    for (i = 0; i < 2; i++)
        for (s = 0; s < TDM_SLOTS; s++)
            for (n = 0; n < FRAME_LEN; n++)
                tx_bufs[i][s][n] = 0;
    rx_index = 0;
    frame_ready = 0;
    if (!(RTI(RTIGCTRL) & 2u)) {                   /* counter 1 off (cold boot): start it */
        RTI(RTIUC1) = 0;
        RTI(RTIFRC1) = 0;
        RTI(RTICPUC1) = 1;
        RTI(RTIGCTRL) |= 2u;
    }
    dmax_audio_init(&rx_bufs[0][0][0][0], &rx_bufs[1][0][0][0], &tx_bufs[0][0][0], &tx_bufs[1][0][0]);
}

void audio_io_on_rx_done(void)
{
    const uint32_t t = RTI(RTIFRC1);
    g_cpu.period = t - isr_t;
    isr_t = t;
    if (frame_ready) g_cpu.lost++;
    frame_ready = rx_index + 1;
    rx_index ^= 1;
}

void audio_io_service(void)
{
    int ready = frame_ready;
    if (ready) {
        uint32_t t0, b;
        frame_ready = 0;
        t0 = RTI(RTIFRC1);
        audio_process((const rx_buf_t *)&rx_bufs[ready - 1], &tx_bufs[ready - 1]);
        b = RTI(RTIFRC1) - t0;
        g_cpu.busy = b;
        g_cpu.avg += (int32_t)(b - g_cpu.avg) >> 4;
        if (b > g_cpu.max) g_cpu.max = b;
    }
}
