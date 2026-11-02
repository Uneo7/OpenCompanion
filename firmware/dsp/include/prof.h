/* Per-stage CPU profile, profiling builds only (make PROF=1 -> -DOC_PROF, output build_prof/; RAM-load test
 * images, they need not fit the EEPROM body). In a normal build every macro is empty: no code, no data.
 * Time base: RTI free-running counter 1 (audio_io.c). PROF_START at the start of audio_process(); each PROF(k)
 * charges the ticks since the previous mark to stage k (smoothed 1/16 per frame). Interrupt handlers add their
 * own duration to PROF_ISR (they also lengthen the stage they interrupt). Console PRF0 / PRF1 (console.c) print
 * the stages in 0.1 % of the frame period. */
#ifndef PROF_H
#define PROF_H

#include <stdint.h>

enum {
    PROF_METER_IN,   /* 0 input fill + input meters (8 channels) */
    PROF_PATH,       /* 1 rate switch, path choice */
    PROF_CTRL,       /* 2 GPIO inputs, volume, supply ceiling */
    PROF_LEVEL,      /* 3 level detectors (loudness / standby powers) */
    PROF_LOUD,       /* 4 standby timer, loudness gains, USB ramp */
    PROF_INPUT,      /* 5 engine controls (matrix, gains) */
    PROF_GEQ,        /* 6 global EQ */
    PROF_GRAPH,      /* 7 preset program (neutral: includes its EQ banks and crossover) */
    PROF_OUTEQ,      /* 8 output EQ bank (Bose), trims, sub level */
    PROF_SAT,        /* 9 satellite band protection (4) */
    PROF_LIMIT,      /* 10 look-ahead limiter */
    PROF_BASS,       /* 11 bass compressor / clipper / delay */
    PROF_GAIN,       /* 12 final gain, supply / clip detector */
    PROF_FINAL,      /* 13 -6 dBFS limiter, fade, PCM, output meters */
    PROF_SPK,        /* 14 speaker mode sequencer */
    PROF_ISR,        /* 15 interrupt handlers (all), per frame */
    PROF_N
};

#ifdef OC_PROF
extern uint32_t g_prof[PROF_N];
extern volatile uint32_t g_prof_t, g_prof_isr_acc;
#define prof_now()      (*(volatile uint32_t *)0x42000030u)          /* RTIFRC1 */
#define PROF_START      (g_prof_t = prof_now())
#define PROF(k)         do { const uint32_t prof_n = prof_now(); \
                             g_prof[k] += (uint32_t)((int32_t)((prof_n - g_prof_t) - g_prof[k]) >> 4); \
                             g_prof_t = prof_n; } while (0)
#define PROF_ISR_IN     const uint32_t prof_isr_t = prof_now();   /* first declaration, no ';' after it */
#define PROF_ISR_OUT    (g_prof_isr_acc += prof_now() - prof_isr_t)
#define PROF_ISR_FRAME  do { g_prof[PROF_ISR] += (uint32_t)((int32_t)(g_prof_isr_acc - g_prof[PROF_ISR]) >> 4); \
                             g_prof_isr_acc = 0; } while (0)
#else
#define PROF_START      ((void)0)
#define PROF(k)         ((void)0)
#define PROF_ISR_IN
#define PROF_ISR_OUT    ((void)0)
#define PROF_ISR_FRAME  ((void)0)
#endif

#endif
