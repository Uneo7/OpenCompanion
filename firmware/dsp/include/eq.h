/* User EQ banks: parametric bands (RBJ cookbook, designed at the current rate) compiled into biquad cascades
 * in the Bose coefficient format (+ the stock crossover on the neutral preset), run by the engine as BG_UEQ ops
 * (src/bose/bose_graph.c); gains (AUX, sub level, trims). Coefficient changes glide over EQ_GLIDE_FRAMES frames
 * (eq_chain.c): no click, no extra filtering while nothing changes. */
#ifndef EQ_H
#define EQ_H

#include "biquad.h"
#include "params.h"

/* Biquad cascade with click-free coefficient changes (eq_chain.c): f runs, its coefficients glide to the target
 * (tc, tg, tn sections) over `steps` more frames. */
#define EQ_GLIDE_FRAMES 16                /* 32 ms at 48 kHz */
typedef struct {
    biquad_t f;
    biquad_coef_t tc[BIQUAD_MAX_SECTIONS];
    float tg;
    uint8_t tn, steps;
} eq_chain_t;

void chain_init(eq_chain_t *c, float g, const biquad_coef_t *sec, int n);
/* New target, glides; returns 1 if the section count changes (re-packed sections: switch behind the ramp). */
int  chain_set(eq_chain_t *c, float g, const biquad_coef_t *sec, int n);
/* One frame of n <= 4 chains in place on x[0..n-1] (one 4-lane pass), gliding first if needed. */
void eq_chain_run(eq_chain_t *c, float *const x[], int n);
extern int eq_hold;                               /* 1: glides wait (engine.c, switch behind the ramp) */
void chain_settle(eq_chain_t *c, int rest);       /* end a glide at once (outputs silent); rest: clear states */
void eq_settle(int rest);                         /* every chain: preset / rate / profile switch, outputs silent */

/* One RBJ band in the Bose format: section (b1, b2, a1, a2) and the gain factor to fold into g.
 * Returns 0 if the band is off or degenerate. */
int eq_design_band(const eq_band_t *b, float fs, biquad_coef_t *sec, float *gain);
/* eq_design.c: compiles the enabled bands of a set (returns the section count). */
int eq_compile(const eq_band_t *bands, biquad_coef_t *sec, float *g);

/* Linear gains derived from the active profile (sub level, trims, AUX gain). */
typedef struct {
    float aux;
    float sub;
    float trim[EQ_OUTPUTS];
} eq_gains_t;
extern eq_gains_t g_eq_gains;

void eq_init(void);
void eq_rebuild(const profile_t *p, uint32_t dirty);
enum { EQ_BANK_GLOBAL, EQ_BANK_OUT };
#define EQ_BANK_LANES 5
/* One frame of bank EQ_BANK_GLOBAL (x[0..1] = L, R) or EQ_BANK_OUT (x[0..4] = inner L, inner R, outer L,
 * outer R, bass), in place. */
void eq_bank_run(int bank, float *const x[]);
/* Preset-dependent compilation: xover = stock crossover in the output lanes (neutral preset), r21 = routage 2.1
 * in effect (outer lanes use the inner bands). Rebuilds the output bank on a change. */
void eq_preset(int xover, int r21);
uint32_t eq_active_sections(void);                   /* for the CPU estimate (SYS / OPI) */

#endif
