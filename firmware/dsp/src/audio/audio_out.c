/* 0.2.4: the OpenCompanion -12 dB volume ceiling and -6 dBFS limiter are gone: the maximum level is the stock
 * one (USB 0 dB, pod 99 % = 0 dB, final gain -1 dB above 10.5 V), with the stock protection (band clippers,
 * compressors, look-ahead limiter 0.95, VRAW ceiling, supply / clip fault). */
#include "oc.h"
#include "audio_out.h"
#include "protect.h"
#include "inputs.h"
#include "rate.h"
#include "prof.h"
#include "kern.h"
#include "engine.h"

#define LIMIT        0.999f              /* anti-wrap only (0.2.4): just below full scale, as the PCM clamp */
#define RELEASE_P48  0.999f              /* limiter release pole per sample (~20 ms) */

static float lim_gain = 1.0f, release_f;
static float ph, ph_step;              /* output ramp phase 0 (silent) .. 1 (open), per-sample step */

#define RAMP_MS 15.0f                  /* mute / unmute / switch ramp */

void audio_out_rate(void)
{
    int i;
    float p = RELEASE_P48, q = 1.0f;
    for (i = 0; i < FRAME_LEN; i++) q *= p;
    release_f = 1.0f - q;                                  /* per frame */
    ph_step = 1000.0f / (RAMP_MS * g_fs);
}

void audio_out_init(void)
{
    audio_out_rate();
    lim_gain = 1.0f;
    ph = 0.0f;                         /* opens with the ramp at start-up */
}

DSP_SCRATCH(h)
static float h[FRAME_LEN];

/* Output ramp, applied at the INPUT of the post-stage (0.2.5). Every switch happens upstream (engine), and the
 * satellites (and the bass on the neutral preset) cross a 320-sample look-ahead delay: with the ramp at the DAC
 * (0.2.2 .. 0.2.4) the step of a switch made at "silence" was still in the delay line and reached the DAC
 * 6.7 ms later, while the ramp was already reopening: a pop at full level on every profile / mode switch,
 * and on every unmute (pod mute = headphone path = preset switch). Now the switch is silent everywhere, and
 * audio_out_silent() also waits for the delay line and the protection filters to flush. */
static int flush;                      /* frames left before the silence has reached the DAC */
#define FLUSH_FRAMES 8u                /* 320 samples of delay + filter tails (16 ms) */

int audio_out_silent(void) { return ph <= 0.0f && flush == 0; }

static void input_ramp(float (*post)[FRAME_LEN], int open)
{
    float ph0 = ph, ph1;
    int k;
    ph1 = ph0 + (open ? 1.0f : -1.0f) * ph_step * (float)FRAME_LEN;
    ph1 = ph1 > 1.0f ? 1.0f : (ph1 < 0.0f ? 0.0f : ph1);
    ph = ph1;
    if (ph1 > 0.0f) flush = FLUSH_FRAMES;
    else if (flush) flush--;
    if (ph0 == 1.0f && ph1 == 1.0f) return;
    k_fade(h, 1.0f, 1.0f, ph0, ph1, FRAME_LEN);
    for (k = 0; k < OUT_COUNT; k++) k_scalev(post[k], post[k], h, FRAME_LEN);
}

/* Full-scale guard on the loudest output, per frame: at most LIMIT / its peak (applied from the first sample:
 * no overshoot), recovering with the ~20 ms release between frames: h[n] for every output. Output meters from
 * the block peak before it, scaled by its end value. Returns 0 if an output is not finite (or absurd): the
 * caller resets the signal path (audio_out). */
static int final_gain(float (*post)[FRAME_LEN])
{
    float pk = 0.0f, p[OUT_COUNT], g0 = lim_gain, g1, sq;
    int k;
    for (k = 0; k < OUT_COUNT; k++) {
        p[k] = k_peaksq(post[k], FRAME_LEN, &sq);
        if (!(sq < 1.0e6f)) return 0;                     /* NaN, Inf or a blown filter (the peak skips NaN) */
        if (p[k] > pk) pk = p[k];
    }
    g1 = g0 + (1.0f - g0) * release_f;                    /* release over the frame */
    if (pk * g1 > LIMIT) {
        g0 = g1 = LIMIT / pk;                             /* attack: the whole frame */
        g_audio.clips++;
    }
    lim_gain = g1;
    k_fade(h, g0, g1, 1.0f, 1.0f, FRAME_LEN);
    for (k = 0; k < OUT_COUNT; k++)
        meters_set(METER_OUT0 + k, p[k] * h[FRAME_LEN - 1]);
    return 1;
}

void audio_out(float (*post)[FRAME_LEN], tx_buf_t *out, int open, int bass_delay)
{
    int k;
    input_ramp(post, open);
    if (!g_audio.headphones) protect_sats(post);     /* headphones: no band protection (stock 0x100276E8) */
    PROF(PROF_SAT);
    protect_limit(post, FRAME_LEN);
    PROF(PROF_LIMIT);
    protect_basses(post[OUT_BASS_A], post[OUT_BASS_B]);
    if (bass_delay)
        for (k = 0; k < 2; k++) protect_delay_bass(k, post[OUT_BASS_A + k], FRAME_LEN);
    PROF(PROF_BASS);
    gpio_in_out_gain(post, OUT_COUNT, FRAME_LEN, g_supply.ceiling);
    supply_detect(post, FRAME_LEN);
    PROF(PROF_GAIN);
    /* Never let a NaN / Inf reach the DAC, never keep a stuck compressor (gain below -100 dB, or 0 / NaN: its
     * input was absurd, k_dyn.c, while its own output looks silent or normal); recover by itself. */
    if (!final_gain(post) || !(protect_comp_gain() > 1.0e-5f)) {
        engine_recover();
        protect_init();
        ph = 0.0f;                                   /* reopen with the ramp */
        k_zero(&post[0][0], OUT_COUNT * FRAME_LEN);
        k_zero(h, FRAME_LEN);
    }
    for (k = 0; k < OUT_COUNT; k++) k_pcm((*out)[k], post[k], h, FRAME_LEN);
    k_zero((float *)(*out)[6], 2 * FRAME_LEN);          /* slots 6, 7: zero (same bit pattern as 0.0f) */
    PROF(PROF_FINAL);
}
