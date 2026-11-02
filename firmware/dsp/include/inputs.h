/* Hardware inputs and their stock control laws: master gain and loudness, bass knob, GPIO inputs (thermistors, clip
 * detect, BADDATA) and the silence standby request. One frame = 96 samples = 2 ms (stock "block"). */
#ifndef INPUTS_H
#define INPUTS_H

#include <stdint.h>

/* --- level_detect.c: stock-scaled channel powers (meter 0x10010368) --- */
#define LD_CHANNELS 6
/* in: the engine inputs (bose_graph_input: USB L, R, C, LFE, Ls, Rs, AUX L, AUX R, full scale = 1 for AUX),
 * scale: 1 / the program's USB input level. */
void level_detect(float *const in[8], float scale, float usb_gain, float pow6[LD_CHANNELS]);

/* --- loudness.c: master gain + stock dynamic loudness (0x1001AF08) --- */
typedef struct {
    float gain_in;     /* master gain before the ceiling, linear */
    float env_db;      /* loudness detector, dB (power) */
    float level;       /* loudness level estimate (stock w20) */
    float boost_db;    /* loudness bass boost, dB (stock w13) */
    float g_direct;    /* target, direct path (gain 0x100103FC) */
    float g_band;      /* target, band-pass path (gain 0x10010430) */
    int   enabled;     /* stock loudness enabled (PF_LOUDNESS) */
} loudness_t;
extern loudness_t g_loud;
void loudness_init(void);
void loudness_rate(void);
void loudness_frame(float master_db, int mute, float cap_lin, int enabled, const float pow6[LD_CHANNELS]);
void loudness_gains(int n);                          /* computes the smoothed per-sample gains */
const float *loudness_gain_direct(void);             /* this frame's per-sample gains (stock graph hooks) */
const float *loudness_gain_band(void);

/* --- gpio_in.c: McASP0 GPIO inputs, thermal gain (0x1001B638), BADDATA selection (selector 0x10010758: the engine programs) --- */
typedef struct {
    uint32_t pdin;       /* last McASP0 PDIN */
    uint8_t  t1, t2;     /* thermistor RT6206 / RT6307 hot (AXR0[4] / AXR0[5] low) */
    uint8_t  cclip;      /* satellite amplifier clip (CCLIP\, AXR0[7] low) */
    uint8_t  bsclip;     /* bass amplifier clip (BSCLIP\, AXR0[8] low) */
    uint8_t  baddata;    /* USB checksum error (BADDATA, AXR0[15] high) */
    uint8_t  sel;        /* BADDATA low-pass selected */
    uint32_t n_hot, n_cclip, n_bsclip, n_baddata;   /* frames with the condition (diagnostics) */
    float    therm_gain; /* stock 0x1001B638 state (1.0 .. 0.22387) */
} gpio_in_t;
extern gpio_in_t g_gpio;
void  gpio_in_init(void);
void  gpio_in_rate(void);
void  gpio_in_frame(void);                           /* once per audio frame */
void  gpio_in_out_gain(float (*out)[96], int nout, int n, float ceiling); /* final gain = ceiling x thermal */

/* --- standby.c: silence standby request (stock 0x10010128 / 0x1001B500 / 'S' reply) --- */
typedef struct {
    float   counter;     /* silent blocks (stock timer state) */
    float   delay;       /* blocks before the request (stock 0x10010128 = 60000 = 120 s) */
    float   power;       /* last detector input (mean L/R block power) */
    float   threshold;   /* activity threshold (stock 7.801579e-8, word 10 of 0x10010230; options bits 5..7) */
    uint8_t request;     /* stock byte 0x540E: 'S' reply byte 1 */
    uint8_t fault;       /* stock byte 0x540F: 'S' reply byte 2 (supply / clip fault, protect_supply.c) */
} standby_t;
extern standby_t g_standby;
void standby_init(void);
void standby_rate(void);
/* Device options byte (OCC1 config record byte 11, profiles.h OPT_*): kept here, applied to the delay and the
 * threshold; read by profiles.c (pod double press) and audio.c (headphone EQ). */
extern int g_dev_opts;
void standby_config(int options);
void standby_frame(const float pow6[LD_CHANNELS], float scale);   /* scale = master gain^2 */

/* --- console_inputs.c --- */
int console_inputs(const uint8_t *msg, int len, uint8_t *out, int max);   /* -1 = not ours */

#endif
