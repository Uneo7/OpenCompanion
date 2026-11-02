/* Host check of the HOST / LOCAL mode decision (volume.c update_mode through volume_frame).
 *   host_mode
 * Scenario of the 0.2.1 hardware bug (speaker restart, TAS 0.4.7): 'U' received, DSPINT stays high while the
 * host streams ('R' layout bit0 = 1). Expected: HOST and USB active for the whole stream; after the stream closes
 * (bit0 = 0) with DSPINT high, LOCAL and USB muted after the 1 s debounce; stream open again -> HOST at once. */
#include <stdio.h>
#include "oc.h"
#include "volume.h"
#include "inputs.h"
#include "ramp.h"
#include "params.h"

audio_state_t g_audio;
gpio_in_t g_gpio;
standby_t g_standby;
ramp_state_t g_ramp;
profile_t g_profile;

static int run(int frames, int open)
{
    int f, bad = 0;
    g_audio.stream_open = open;
    for (f = 0; f < frames; f++) {
        volume_frame();
        bad |= open ? (!g_vol.host || g_vol.usb_off) : 0;
    }
    return bad;
}

int main(void)
{
    int fail = 0;
    g_audio.volume_db = -19.0f;
    g_audio.pod_present = 1;
    g_ramp.pod_pct = 35.0f;
    volume_init();
    g_audio.usb_up = 1;                   /* 'U' */
    g_gpio.pdin = 1u << 9;                /* DSPINT high: the TAS does not pull it low */
    fail |= run(1500, 1);                 /* 3 s of open stream */
    printf("stream open, DSPINT high: host=%d usb_off=%d master=%.1f dB\n", g_vol.host, g_vol.usb_off, g_vol.master_db);
    run(600, 0);
    printf("stream closed, DSPINT high, 1.2 s: host=%d usb_off=%d master=%.1f dB\n", g_vol.host, g_vol.usb_off,
           g_vol.master_db);
    fail |= g_vol.host || !g_vol.usb_off;
    fail |= run(1, 1);
    printf("stream open again: host=%d usb_off=%d -> %s\n", g_vol.host, g_vol.usb_off, fail ? "FAIL" : "OK");
    return fail;
}
