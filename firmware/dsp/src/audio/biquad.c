#include "biquad.h"
#include "kern.h"
#include "oc.h"

void biquad_init(biquad_t *bq, float g, const biquad_coef_t *sections, int nsec)
{
    int i;
    if (nsec > BIQUAD_MAX_SECTIONS) nsec = BIQUAD_MAX_SECTIONS;
    bq->g = g;
    bq->nsec = nsec;
    for (i = 0; i < nsec; i++) {
        bq->c[i] = sections[i];
        bq->z[i][0] = bq->z[i][1] = 0.0f;
    }
}

static float idle_z[3][2];                  /* scratch state of the zero sections (stays 0) */
static const float zero_sec[4] = { 0.0f, 0.0f, 0.0f, 0.0f };

/* Four cascades at once through the 4-lane kernel: out[l] = bq[l](in[l]) (out[l] may be in[l]; the four
 * outputs distinct). A null bq[l] is an idle lane (out[l] is then scratch). Shorter cascades run zero
 * sections (identity) for the remaining sections. */
void biquad_run4(biquad_t *const bq[4], const float *const in[4], float *const out[4], int n)
{
    const float *cc[4];
    float *zz[4];
    int l, s, ns = 0;
    for (l = 0; l < 4; l++) {
        if (bq[l]) {
            k_scale(out[l], in[l], bq[l]->g, n);
            if (bq[l]->nsec > ns) ns = bq[l]->nsec;
        }
    }
    for (s = 0; s < ns; s++) {
        for (l = 0; l < 4; l++) {
            const int real = bq[l] && s < bq[l]->nsec;
            cc[l] = real ? &bq[l]->c[s].b1 : zero_sec;
            zz[l] = real ? bq[l]->z[s] : idle_z[l < 3 ? l : 0];
        }
        k_sec4(out, cc, zz, n);
    }
}
