/* Host harness for the neutral program of the engine (src/bose/bose_progs.c) with the real user EQ banks
 * (eq.c: flat bands + the stock crossover): runs the firmware C code on a PC, frame by frame.
 *   host_engine IN OUT
 * IN:  int32 frame count, then per frame: float usb, loud_d, loud_b, baddata, then 8 x 96 float inputs.
 * OUT: per frame 5 x 96 float outputs (inner L, inner R, outer L, outer R, bass), before trims / sub level.
 * Layout 5.1 (matrix: L + 0.7071 C + 0.7071 Ls, ...), AUX gain 1, 48 kHz (rate stubs as host_bose_graph.c). */
#include <stdio.h>
#include <string.h>
#include <math.h>
#include "bose_graph.h"
#include "biquad.h"
#include "rate.h"
#include "eq.h"

uint8_t g_layout = LAYOUT_51;
profile_t g_profile;
float mx_sqrt(float x) { return sqrtf(x); }
float mx_exp2(float x) { return exp2f(x); }
float mx_log2(float x) { return log2f(x); }
int bose_tables_ok(void) { return 1; }
void engine_profile_swap(void) { }
int audio_graph_active(void) { return 0; }
int eq_compile(const eq_band_t *bands, biquad_coef_t *sec, float *g)
{
    (void)bands; (void)sec;
    *g = 1.0f;
    return 0;                                   /* flat user bands */
}

int main(int argc, char **argv)
{
    static const float mtx[14] = { 1.0f, 0.70710678f, 0.70710678f, 1.0f, 0.70710678f, 0.70710678f, 1.0f,
                                   1.0f, 1.0f, 1.0f, 1.0f, 1.0f, 0.5f, 0.5f };
    FILE *fi, *fo;
    int32_t nf;
    int f, k, i;
    float ctl4[4], usb[FRAME_LEN], ld[FRAME_LEN], lb[FRAME_LEN];
    bg_ctl_t ctl;
    if (argc < 3 || !(fi = fopen(argv[1], "rb")) || !(fo = fopen(argv[2], "wb"))) return 2;
    if (fread(&nf, 4, 1, fi) != 1) return 3;
    memset(&g_profile, 0, sizeof(g_profile));
    eq_init();
    bose_graph_select(&bg_prog_neutral);
    eq_preset(1, 0);
    for (f = 0; f < nf; f++) {
        if (fread(ctl4, 4, 4, fi) != 4) return 4;
        for (k = 0; k < BG_IN_COUNT; k++)
            if (fread(bose_graph_input(k), 4, FRAME_LEN, fi) != FRAME_LEN) return 5;
        for (i = 0; i < FRAME_LEN; i++) { usb[i] = ctl4[0]; ld[i] = ctl4[1]; lb[i] = ctl4[2]; }
        ctl.usb = usb; ctl.loud_d = ld; ctl.loud_b = lb; ctl.baddata = ctl4[3]; ctl.mtx = mtx; ctl.ueq = ~0u;
        bose_graph_run(&ctl);
        for (k = 0; k < 5; k++) fwrite(bose_graph_output(k), 4, FRAME_LEN, fo);
    }
    fclose(fo);
    return 0;
}
