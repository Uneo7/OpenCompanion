/* Block kernels: every per-sample loop of the audio path, built for speed (Makefile SPEED_SRCS: software
 * pipelined, no per-sample branch). One call = one block of n samples (n = FRAME_LEN unless stated; the
 * MUST_ITERATE hints assume a multiple of 16, >= 16, except k_copy and the short delay lines). The structure follows the stock block loops
 * (re/dsp/out/img.dis: vector multiply 0x1002D880 ii = 1, multiply-add 0x1002D900 ii = 2): scalar gains stay in
 * registers, no gain array is filled for a constant gain.
 *
 * Aliasing: y may be the same buffer as an input of an element-wise kernel (each element is read before it is
 * written); never a partial overlap. Kernels documented "y != x" need distinct buffers. */
#ifndef KERN_H
#define KERN_H

#include <stdint.h>

/* Block scratch buffers (written before every read) and states cleared by an init function go to .dsp_ram in IRAM_HI (lnk.cmd: uninitialised, not in
 * the image): DSP_SCRATCH(buf) before the definition "static float buf[...]". */
#ifdef __TI_COMPILER_VERSION__
#define DSP_PRAGMA(x) _Pragma(#x)
#define DSP_SCRATCH(sym) DSP_PRAGMA(DATA_SECTION(sym, ".dsp_ram"))
#else
#define DSP_SCRATCH(sym)
#endif

/* --- k_vec.c --- */
void  k_scale(float *y, const float *x, float t, int n);               /* y = x t, any n >= 1 */
void  k_mac(float *y, const float *x, float t, int n);                 /* y += x t */
/* Copy, any n >= 1: a multiply by a run-time 1 (exact), because a plain copy loop is turned into a call to
 * memcpy, whose byte loop (libc_min.c) is ~10x slower. */
/* Block power, through the peak / power kernel. */
#define k_sumsq(x, n) k_power((x), (n))
float k_power(const float *x, int n);
#define k_copy(y, x, n) k_scale((y), (x), 1.0f, (n))
void  k_zero(float *y, int n);
void  k_i2f(float *y, const int32_t *x, float s, int n);               /* y = (float)x s */
void  k_scalev(float *y, const float *x, const float *g, int n);       /* y = x g[i] */
void  k_macv(float *y, const float *x, const float *g, int n);         /* y += x g[i] */
float k_peaksq(const float *x, int n, float *sumsq);                   /* max |x|, and sum x^2 */
void  k_pcm(int32_t *pcm, float *x, const float *h, int n);            /* x *= h; pcm = 24-bit of clamp(x) */
/* Delay line of len samples (len >= 1). Circular buffer line[len], position *pos; y != x. */
void  k_delay(float *line, int len, int *pos, const float *x, float *y, int n);

/* --- k_biquad.c --- */
/* One section on 4 channels: transposed direct form II, Bose coefficient convention (c[l] = b1, b2, a1, a2;
 * z[l] = 2 states), in place on y[l]. Lanes must be distinct buffers. */
void  k_sec4(float *const y[4], const float *const c[4], float *const z[4], int n);

/* --- k_dyn.c --- */
/* Stock soft clipper (class 0x1002D0A8): a = min(|x|, hi), y = sign(x) (a - c max(a - lo, 0)^2), in place. */
void  k_softclip(float *x, float lo, float hi, float c, int n);
/* Stock compressor (class 0x1002CB4C) on 4 channels at once (unused lanes: any scratch buffer, gain
 * ignored). p = { P0, P1, P2, P4 }; g[4] = gains, updated; in place. */
void  k_comp4(float *const x[4], float g[4], const float p[4], int n);
/* Peak follower with per-sample decay: p = max(p d, |x|); returns the new p. */
float k_decay_peak(const float *x, float p, float d, int n);
/* One-pole gain smoother of the stock mixers (g = b (t + t_prev) + p g), constant target t over the block:
 * g[i] for i = 0..n-1; *cur and *prev updated. */
void  k_smooth(float *g, float *cur, float *prev, float t, float b, float p, int n);
/* Output ramp: linear a -> b times a smoothstep of the phase p0 -> p1 (audio_out.c, 15 ms open / close). */
void  k_fade(float *g, float a, float b, float p0, float p1, int n);
/* Cubic Hermite gain of one limiter sub-block: g[i] = s0 + t (m0 + t (c2 + t c3)), t = (i + 1) dt. Returns
 * min g. */
float k_hermite(float *g, float s0, float m0, float c2, float c3, float dt, int n);

#endif
