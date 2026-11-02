#include "oc.h"
#include "volume.h"
#include "volume_curve.h"
#include "inputs.h"
#include "ramp.h"
#include "params.h"

#define DSPINT_BIT     (1u << 9)
/* Frames of a stable DSPINT level before the mode changes: 1 s, longer than the ~0.5 s the TAS (0.4.0) holds
 * DSPINT high while it switches between the 5.1 and 2.1 modes (re-enumeration), so the volume source does
 * not flip to the pod knob and back; the USB channels are still muted at once while it is high (stock). */
#define MODE_DEBOUNCE  500u
#define KNOB_SETTLE    100u         /* ramp frames before the knob is reported (smoothing 0.95 per frame) */

volume_t g_vol;
static uint32_t mode_count;
static uint8_t last_mute;
static int knob_dir;                /* knob_step(): direction of the last move */
static uint32_t ramp_seen;          /* g_ramp.frames at the last audio frame */
static uint8_t ramp_idle;           /* audio frames without a new ramp frame */

void volume_init(void)
{
    g_vol.host = 1;
    g_vol.usb_off = 0;
    g_vol.knob = 0;
    g_vol.knob_valid = 0;
    g_vol.events = 0;
    g_vol.master_db = g_audio.volume_db;
    mode_count = 0;
    last_mute = 0;
}

/* USB volume law: the UAC dB value of 'V' in every profile, so the OS slider means the same level on the Bose
 * and the neutral base (PF_VOL_STOCK is kept in the profile format but ignored: the stock % curve made an OS
 * -30 dB play at -5.4 dB). The stock loudness follows this master (loudness.c); the stock graph needs no
 * percentage: its level inputs are the loudness gains. VOL_SAFE_DB until the first 'V'. */
void audio_update_volume(void)
{
    if (!g_audio.volume_known) {
        g_audio.volume_db = VOL_SAFE_DB;
    } else {
        float db = (float)g_audio.volume_raw / 256.0f;    /* UAC: 1/256 dB, 0x8001 = about -128 dB */
        if (db < -80.0f) db = -80.0f;
        if (db > 0.0f) db = 0.0f;
        g_audio.volume_db = db;
    }
}

void audio_set_volume_raw(int16_t v)
{
    g_audio.volume_raw = v;
    g_audio.volume_known = 1;
    g_audio.volume_pct = volume_pct_from_uac(v);           /* stock 'V' handler -> 0x10010100 */
    audio_update_volume();
}

/* Stock 0x10015700: only once 'U' was received and no standby is requested; pin high = USB inactive. An open
 * stream ('R' layout bit0, TAS >= 0.4.0) means HOST mode and USB active whatever DSPINT reads: on hardware the
 * line can stay high while the host streams, which left the USB channels muted. */
static void update_mode(void)
{
    int inactive;
    if (g_audio.stream_open) {
        g_vol.usb_off = 0;
        g_vol.host = 1;
        mode_count = 0;
        return;
    }
    if (!g_audio.usb_up || g_standby.request) return;
    inactive = (g_gpio.pdin & DSPINT_BIT) != 0;
    g_vol.usb_off = (uint8_t)inactive;
    if (inactive == !g_vol.host) {
        mode_count = 0;
    } else if (++mode_count >= MODE_DEBOUNCE) {
        g_vol.host = (uint8_t)!inactive;
        mode_count = 0;
    }
}

static void update_knob(void)
{
    const float p = g_ramp.pod_pct;
    int k;
    uint8_t m = (uint8_t)(g_audio.pod_mute != 0);
    /* Valid only with a pod reported ('X' != 0), a settled filter, a plausible IVOL reading (a floating input
     * is never reported: the TAS 0.5.2+ pushes the knob only when this bit is set, and cannot see a hot-unplug
     * of an unmuted pod) and fresh ramp frames (none for 50 ms: the converter stopped). */
    if (g_ramp.frames != ramp_seen) { ramp_seen = g_ramp.frames; ramp_idle = 0; }
    else if (ramp_idle < 255u) ramp_idle++;
    g_vol.knob_valid = (uint8_t)(g_audio.pod_present && g_ramp.frames > KNOB_SETTLE && g_ramp.ivol_ok &&
                                 ramp_idle < 25u);
    if (m != last_mute) {
        last_mute = m;
        g_vol.events++;
    }
    if (!g_vol.knob_valid) return;
    k = knob_step(p, g_vol.knob, &knob_dir);
    if (k != g_vol.knob) {
        g_vol.knob = (uint8_t)k;
        g_vol.events++;
    }
}

/* HOST mode: the OS volume alone (the TAS reports the pod knob to the host, which moves the OS volume: the knob
 * must not act a second time here). LOCAL mode (no USB host): the pod volume (stock-filtered knob) through the
 * stock pod curve 0x10010564; without a pod, the last USB volume. Smoothed per sample by the loudness gains. */
void volume_frame(void)
{
    update_mode();
    update_knob();
    if (!g_vol.host && g_audio.pod_present)                             /* local: pod knob, the reported value */
        g_vol.master_db = volume_curve_db(&VC_STOCK_POD, g_vol.knob_valid ? (float)g_vol.knob : g_ramp.pod_pct);
    else
        g_vol.master_db = g_audio.volume_db;                                /* OS master volume */
}

int volume_status(uint8_t *st)
{
    st[0] = (uint8_t)(VS_EXTENDED | (g_audio.pod_present ? VS_POD_PRESENT : 0) |
                      (g_audio.pod_mute ? VS_POD_MUTE : 0) | (g_vol.host ? 0 : VS_LOCAL) |
                      (g_vol.knob_valid && g_audio.pod_present ? VS_KNOB_VALID : 0));   /* 'X' 00: cleared at once */
    st[1] = g_vol.knob;
    st[2] = g_vol.events;
    return 3;
}
