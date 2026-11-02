/* Silence standby request, port of the stock chain:
 *   0x100101D0 (class 0x1001C820): x = mean of meter outputs 0x1000916C, 0x10009170 = block powers of the
 *               mixed L and R channels (level_detect.c), (1/n computed in double, 0x1001C84C)
 *   0x10010200 (0x1001C820): copies the delay 0x10010128 (60000.0 blocks = 120 s) into word 11 of the timer
 *   0x10010230 (class 0x1001B500): if x > 7.801579e-8 (word 10, 0x33A789A3, about -71 dBFS RMS) the counter
 *               is cleared, else it counts up to delay + 1; output (0x10010110) = 1.0 when counter > delay
 *   0x100157E8: when 0x10010110 != 0: 0x1001010C = 1.0 (fault/standby input of 0x1001C780, which mutes the
 *               pod curve), McASP0 PDIR bit 14 cleared (spk_mode.c), byte 0x540E = 1. Never cleared: the TAS
 *               resets the DSP. Stock also requires 0x10015700 to return 1 (AXR0[9] high for 0x10006D68 blocks).
 *   'S' poll (task code 5, 0x10016030): reply = 2 bytes [0x540E][0x540F]. TAS PollDspStatus (u6002 code
 *               0x1128..0x1176): either byte != 0 -> return 1 -> EnterSuspend (0x125F); byte 2 != 0 also sets
 *               TAS bit 0x13. 0x540F is set by the VRAW protection (0x10015910), not ported: always 0 here.
 * The stock counter depends on the USB volume (measured after it) but not on the pod volume. OpenCompanion
 * measures before the master volume and scales by the master gain^2 (scale), so with the OS volume as master
 * the behaviour is the stock one with no pod.
 * OpenCompanion difference: stock never clears the request (the TAS powers the system down and resets the DSP).
 * Here it is cleared as soon as the detector sees activity again (not after a supply / clip fault, which stays
 * latched): when the TAS keeps the DSP running (USB active), a request raised during 120 s of silence or of a
 * very low master volume (the detector is scaled by the master gain^2) would otherwise mute the outputs for good.
 * spk_mode.c drives the amplifier again. */
#include "oc.h"
#include "inputs.h"
#include "rate.h"
#include "profiles.h"

#define SILENCE_THRESHOLD 7.801579e-8f
#define STANDBY_OFF       1.0e9f          /* "never": the float counter stops at 2^24 blocks */

standby_t g_standby;
int g_dev_opts;                           /* 0 = stock */

void standby_init(void)
{
    g_standby.counter = 0.0f;
    standby_rate();
    g_standby.power = 0.0f;
    g_standby.request = 0;
    g_standby.fault = 0;
}

void standby_config(int options)
{
    g_dev_opts = options;
    standby_rate();
}

/* Delay (OPT_DELAY: 2 min stock, off, 5, 10, 20, 30, 60 min; 30000 blocks per minute at 48 kHz) and threshold
 * (options bits 5..7: stock x 4^n power, n = -4..+3, 6.02 dB per step: -95.2..-53.0 dBFS, 0 = stock -71.1 dBFS). */
void standby_rate(void)
{
    static const uint8_t MINUTES[8] = { 2, 0, 5, 10, 20, 30, 60, 2 };
    const uint32_t m = MINUTES[g_dev_opts & OPT_DELAY];
    g_standby.delay = m ? (float)m * 30000u : STANDBY_OFF;
    g_standby.threshold = SILENCE_THRESHOLD / 256.0f * (float)(1u << ((((int8_t)g_dev_opts >> 5) + 4) * 2));
}

void standby_frame(const float pow6[LD_CHANNELS], float scale)
{
    float x = (pow6[0] + pow6[1]) * 0.5f * scale;
    g_standby.power = x;
    if (x > g_standby.threshold) {
        g_standby.counter = 0.0f;
        if (!g_standby.fault) g_standby.request = 0;   /* OpenCompanion: activity cancels it (below) */
    }
    else if (g_standby.counter < g_standby.delay + 1.0f)
        g_standby.counter += 1.0f;
    if (g_standby.counter > g_standby.delay)
        g_standby.request = 1;          /* latched (stock byte 0x540E) */
}
