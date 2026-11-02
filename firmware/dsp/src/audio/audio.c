/* Master volume: the OS volume ('V', UAC dB) in HOST mode, the pod knob through the stock pod curve in LOCAL
 * mode. It acts after the USB + AUX mix, so it controls AUX too. The USB mute 'm' (and USB inactive, stock
 * 0x10015744) mutes the USB channels only, as stock; a standby request and a supply fault mute everything. Those
 * two and the pod mute 'M' release the amplifier control AXR0[14] (spk_mode.c); 'm' does not.
 * Headphones ('M' = the MUTE line: mute touch or headphones plugged, indistinguishable): the headphone amplifiers
 * U1000 are fed by DAC_LC / DAC_RC = TDM slots 1 / 2 = inner L / R and cannot be muted. 'M' 01 mutes the master
 * at once until the sequencer has released the amplifiers (MD 1 / 3), then the feed fades in on inner L / R
 * (neutral program, no output EQ, crossover, trims or band protection, as stock 0x100276E8; the global EQ only
 * with OPT_HP_EQ). 'M' 00: muted while the sequencer switches (MD 0), speakers back from MD 2. The stock
 * headphone mixer gains (0x10010C7C / 0x10010CA4, -2 dB) are not reproduced. */
#include "oc.h"
#include "inputs.h"
#include "volume.h"
#include "protect.h"
#include "rate.h"
#include "mathx.h"
#include "audio_out.h"
#include "volume_curve.h"
#include "spk_mode.h"
#include "prof.h"
#include "engine.h"
#include "sub_level.h"
#include "eq.h"
#include "kern.h"
#include "bose_graph.h"

#define GAIN_P48     0.995f              /* USB mute smoothing pole per sample */
#define MASTER_MAX_DB 5.0f               /* stock 0x1001BB6C pre-gain cap (w29) */
audio_state_t g_audio;

static float gain_cur, gain_p;
static int muted;                        /* pod mute (speakers), switching, standby, fault: output ramp closed */
static float post[OUT_COUNT][FRAME_LEN];
static float usb_gain[FRAME_LEN];

void audio_rate(void)
{
    gain_p = GAIN_P48;
    audio_out_rate();
}

void audio_init(void)
{
    profile_t p;
    /* Conservative until the TAS sends 'V' (a warm start, e.g. a JUMP from rescue or a DSP-only reset, gets no
     * 'V' / 'M' / 'X' / 'R' from the TAS): VOL_SAFE_DB under every volume law (volume.c). */
    g_audio.volume_raw = (int16_t)(-30 * 256);
    g_audio.volume_pct = volume_pct_from_uac(g_audio.volume_raw);
    g_audio.volume_known = 0;
    g_audio.volume_db = VOL_SAFE_DB;
    g_audio.usb_up = 0;
    g_audio.stream_open = 0;
    g_audio.cap_db = 0.0f;           /* 0.2.4: no ceiling below stock (stock maximum = USB 0 dB, pod 99 % = 0 dB) */
    g_audio.usb_mute = 0;
    g_audio.pod_mute = 0;
    g_audio.pod_present = 0;
    g_audio.frames = 0;
    g_audio.clips = 0;
    rate_init();
    audio_rate();
    eq_init();
    loudness_init();
    sub_level_init();
    gpio_in_init();
    standby_init();
    spk_mode_init();
    supply_init();
    protect_init();
    audio_out_init();
    params_defaults(&p);
    params_set_active(&p);           /* flat profile; profiles_init() may replace it */
    volume_init();
    meters_init();
    engine_init();
    gain_cur = 0.0f;                 /* fade in at start-up */
}

static void usb_ramp(float usb_target)       /* USB mute, USB channels only (stock gain 0x100106C8) */
{
    float prev = usb_target;                 /* one-pole g = t + (g - t) p */
    k_smooth(usb_gain, &gain_cur, &prev, usb_target, (1.0f - gain_p) * 0.5f, gain_p, FRAME_LEN);
}

/* Control part of the frame: modes, master gain, loudness, standby. Returns the USB channel gain target. */
static float control(float *const x[BG_IN_COUNT], float usb_scale)
{
    const int bypass = (g_profile.flags & PF_BYPASS) != 0;
    float pow6[LD_CHANNELS], usb_target, master_db, m_lin;
    gpio_in_frame();
    volume_frame();
    supply_frame();
    PROF(PROF_CTRL);
    usb_target = (g_audio.usb_mute || g_vol.usb_off) ? 0.0f : 1.0f;
    level_detect(x, usb_scale, usb_target, pow6);
    PROF(PROF_LEVEL);
    m_lin = g_vol.host ? mx_db_to_lin(g_audio.volume_db) : 1.0f;   /* stock: after the USB volume, not the pod */
    standby_frame(pow6, m_lin * m_lin);
    master_db = g_vol.master_db + g_supply.boost_db;
    if (master_db > MASTER_MAX_DB) master_db = MASTER_MAX_DB;
    g_audio.headphones = g_spk.state == SPK_TO_MUTED || g_spk.state == SPK_MUTED;   /* amplifiers released */
    muted = (!g_audio.headphones && (g_audio.pod_mute || g_spk.state == SPK_SWITCHING)) ||
            g_standby.request || g_supply.fault;
    loudness_frame(master_db, 0, mx_db_to_lin(g_audio.cap_db),
                   (g_profile.flags & PF_LOUDNESS) && !bypass, pow6);
    loudness_gains(FRAME_LEN);
    return usb_target;
}

void audio_process(const rx_buf_t *in, tx_buf_t *out)
{
    float *x[BG_IN_COUNT], fade, pf, s;
    int m;
    PROF_START;
    pf = engine_frame_gate();                       /* preset change: before the inputs are filled */
    s = engine_fill(in, x);                         /* inputs, normalised: USB x s = full scale 1 */
    for (m = 0; m < METER_IN_COUNT; m++)
        meters_update(m, x[m], FRAME_LEN, m < BG_IN_AUX_L ? s : 1.0f);
    PROF(PROF_METER_IN);
    fade = rate_frame();                            /* input layout switch (5.1 / 2.1) */
    if (pf < fade) fade = pf;
    PROF(PROF_PATH);
    usb_ramp(control(x, s));
    PROF(PROF_LOUD);
    engine_run(usb_gain, post);
    PROF(PROF_OUTEQ);
    audio_out(post, out, fade >= 1.0f && !muted, engine_bass_delay());
    spk_mode_frame();                               /* AXR0[14], after the outputs (stock 0x10015E8C) */
    PROF(PROF_SPK);
    PROF_ISR_FRAME;
    g_audio.frames++;
}
