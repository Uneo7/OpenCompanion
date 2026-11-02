/* Speaker / headphone mode sequencer and AXR0[14] (include/spk_mode.h). Decoded from re/dsp/out/img.dis.
 *
 * 1. Mode selector 0x1001BF14 (instance 0x10010164: word 10 = state S, word 12 = T1 = 30, word 13 = T2 = 7,
 *    word 14 = counter C). in = trunc(0x10010114), ack = trunc(0x1001011C), both read on entry:
 *      in == 1 (muted), S != 3 (0x1001BF48..0x1001C0E4):
 *        S == 2 or 4 -> ++C > T1 ? C = 0, S = 0                           0x1001BF7C..0x1001BFBC
 *        S == 0      -> headphone routing; ++C > T2 ? C = 0, S = 1        0x1001BFC0..0x1001C08C
 *        S == 1 and ack == 1 -> ++C > T1 ? C = 0, S = 3                   0x1001C090..0x1001C0E4
 *      in == 2 (normal), S != 4 (0x1001C0E8..0x1001C23C): the same with S == 1 or 3 -> 0, 0 -> 2 (speaker
 *        routing), S == 2 and ack == 0 -> 4.
 *    The three steps run one after the other in the same block (a state reached in one step is handled by
 *    the next step at once). Output 1 = (float)S is copied to MD 0x10010120 (destination counts at
 *    0x10008DA8: output 0 has none, output 1 -> 0x10010120, output 2 -> 0x10010124; copy loop 0x100274C0).
 *    Output 0 (1.0 while switching, 1.0 for the first 50 blocks) has no destination. The routing outputs
 *    3..12 are not used here: OpenCompanion mutes the speakers on 'M' in loudness.c.
 * 2. 0x100157E8, after the graph, with MD = trunc(0x10010120):
 *      standby part (0x100157EC..0x1001584C): if 0x10015700 returns 1 (the USB-inactive timer, see
 *        standby.c / volume.c) and 0x10010110 != 0: 0x1001010C = 1.0, PDIR &= ~0x4000, byte 0x540E = 1;
 *      MD == 1 and byte 0x540C == 0: PDIR &= ~0x4000, 0x1001011C = 1.0, 0x540C = 1   0x100158D4..0x10015904
 *      MD == 2 and byte 0x540D == 0: PDCLR = 0x4000, PDIR |= 0x4000, 0x1001011C = 0.0, 0x540D = 1
 *                                                                                     0x10015880..0x100158D0
 *      any other MD: 0x540C = 0x540D = 0                                              0x10015908..0x1001590C
 * 3. 0x10015910: (0x10010150 != 0 || 0x10010154 != 0) && byte 0x540F == 0 -> 0x1001010C = 1.0,
 *    PDIR &= ~0x4000, 0x540E = 0x540F = 1 (protect_supply.c sets g_standby.request / fault).
 * Initial state (cinit, main 0x10019EB4..0x10019F28): S = C = 0, 0x1001011C = 0, bytes 0x540C..0x5410 = 0,
 * AXR0[14] = input (reset value). Stock starts with 0x10010114 = 1.0 and so keeps the speakers off until the
 * TAS sends 'M' 00 (InitDspLink, 50 ms after the link is up). OpenCompanion starts from g_audio.pod_mute
 * (0 on a cold boot, the stock value after a RAM-loader start, handoff.c): after a DSP-only reset the TAS
 * does not send 'M' again, so waiting for it could leave the speakers off.
 * OpenCompanion differences: the standby release follows g_standby.request (OpenCompanion sets it on silence
 * alone, standby.c), and the stock block counts are kept as block counts (2 ms blocks). */
#include "oc.h"
#include "inputs.h"
#include "spk_mode.h"

#define T1  30u     /* stock instance word 12 */
#define T2  7u      /* stock instance word 13 */

spk_mode_t g_spk;

void spk_mode_init(void)
{
    g_spk.state = SPK_SWITCHING;
    g_spk.ack = 0;
    g_spk.rel_done = 0;
    g_spk.drv_done = 0;
    g_spk.standby_done = 0;
    g_spk.fault_done = 0;
    g_spk.driven = 0;
    g_spk.count = 0;
}

static void step(uint8_t from, uint32_t limit, uint8_t to)
{
    if (g_spk.state != from) return;
    if (++g_spk.count > limit) {
        g_spk.count = 0;
        g_spk.state = to;
    }
}

/* 0x1001BF14 */
static void selector(void)
{
    const int muted = g_audio.pod_mute != 0;
    const uint8_t ack = g_spk.ack;
    if (muted && g_spk.state != SPK_MUTED) {
        if (g_spk.state == SPK_TO_ON || g_spk.state == SPK_ON) {
            if (++g_spk.count > T1) {
                g_spk.count = 0;
                g_spk.state = SPK_SWITCHING;
            }
        }
        step(SPK_SWITCHING, T2, SPK_TO_MUTED);
        if (ack == 1) step(SPK_TO_MUTED, T1, SPK_MUTED);
    } else if (!muted && g_spk.state != SPK_ON) {
        if (g_spk.state == SPK_TO_MUTED || g_spk.state == SPK_MUTED) {
            if (++g_spk.count > T1) {
                g_spk.count = 0;
                g_spk.state = SPK_SWITCHING;
            }
        }
        step(SPK_SWITCHING, T2, SPK_TO_ON);
        if (ack == 0) step(SPK_TO_ON, T1, SPK_ON);
    }
}

static void release(void)
{
    mcasp0_amp_release();
    g_spk.driven = 0;
}

void spk_mode_frame(void)
{
    selector();
    /* 0x100157E8 */
    if (g_standby.request && !g_standby.fault && !g_spk.standby_done) {
        g_spk.standby_done = 1;
        release();
    } else if (!g_standby.request && g_spk.standby_done) {   /* request cancelled by activity (standby.c) */
        g_spk.standby_done = 0;
        if (g_spk.state == SPK_ON) {
            mcasp0_amp_drive_low();
            g_spk.driven = 1;
        }
    }
    if (g_spk.state == SPK_TO_MUTED) {
        if (!g_spk.rel_done) {
            release();
            g_spk.ack = 1;
            g_spk.rel_done = 1;
        }
    } else if (g_spk.state == SPK_TO_ON) {
        if (!g_spk.drv_done) {
            mcasp0_amp_drive_low();
            g_spk.driven = 1;
            g_spk.ack = 0;
            g_spk.drv_done = 1;
        }
    } else {
        g_spk.rel_done = 0;
        g_spk.drv_done = 0;
    }
    /* 0x10015910 */
    if (g_standby.fault && !g_spk.fault_done) {
        g_spk.fault_done = 1;
        release();
    }
}
