/* Host harness for the "Bose d'origine" engine (src/bose/): runs the firmware C code on a PC, frame by frame.
 * Built and driven by re/dsp/bose_graph_check.py, which compares the outputs with re/dsp/stock_sim.py.
 *   host_bose_graph IN OUT
 * IN:  int32 frame count, then per frame: float usb, loud_d, loud_b, baddata, then 8 x 96 float inputs
 *      (USB L, R, C, LFE, Ls, Rs, AUX L, AUX R).
 * OUT: per frame 5 x 96 float outputs (inner L, inner R, outer L, outer R, bass) and the spatial width.
 * 48 kHz only: the rate module is stubbed (identity re-mapping); mx_sqrt (mathx.c, C6x intrinsics) = libm. */
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include "bose_graph.h"
#include "biquad.h"
#include "rate.h"
#include "inputs.h"
#include "eq.h"

float mx_sqrt(float x) { return sqrtf(x); }
void eq_bank_run(int bank, float *const x[]) { (void)bank; (void)x; }   /* built-in program only */

int main(int argc, char **argv)
{
    FILE *fi, *fo;
    int32_t nf;
    int f, k, i;
    float ctl4[4], usb[FRAME_LEN], ld[FRAME_LEN], lb[FRAME_LEN];
    bg_ctl_t ctl;
    if (argc < 3 || !(fi = fopen(argv[1], "rb")) || !(fo = fopen(argv[2], "wb"))) return 2;
    if (fread(&nf, 4, 1, fi) != 1) return 3;
    bose_graph_select(&bg_prog_bose);
    for (f = 0; f < nf; f++) {
        if (fread(ctl4, 4, 4, fi) != 4) return 4;
        for (k = 0; k < BG_IN_COUNT; k++)
            if (fread(bose_graph_input(k), 4, FRAME_LEN, fi) != FRAME_LEN) return 5;
        for (i = 0; i < FRAME_LEN; i++) { usb[i] = ctl4[0]; ld[i] = ctl4[1]; lb[i] = ctl4[2]; }
        ctl.usb = usb; ctl.loud_d = ld; ctl.loud_b = lb; ctl.baddata = ctl4[3]; ctl.mtx = 0; ctl.ueq = 0;
        bose_graph_run(&ctl);
        for (k = 0; k < 5; k++) fwrite(bose_graph_output(k), 4, FRAME_LEN, fo);
        fwrite(&g_spatial.width, 4, 1, fo);
    }
    fclose(fo);
    return 0;
}
