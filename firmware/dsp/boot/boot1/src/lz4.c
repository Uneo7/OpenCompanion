/* LZ4 block decoder (see lz4.h). Sequence: token (literal length in the high nibble, match length - 4 in the
 * low nibble, 15 = more length bytes follow, each 255 = continue), literals, 2-byte little-endian offset,
 * match. The last sequence has literals only. */
#include "lz4.h"

static int length(uint32_t *len, lz4_getc_fn in, void *ctx)
{
    int c;
    if (*len != 15) return 0;
    do {
        if ((c = in(ctx)) < 0) return -1;
        *len += (uint32_t)c;
    } while (c == 255);
    return 0;
}

int lz4_decode(volatile uint8_t *dst, uint32_t raw, lz4_getc_fn in, void *ctx)
{
    uint32_t o = 0, lit, ml, off;
    int tok, lo, hi, c;
    while (o < raw) {
        if ((tok = in(ctx)) < 0) return -1;
        lit = (uint32_t)tok >> 4;
        if (length(&lit, in, ctx) || lit > raw - o) return -1;
        while (lit--) {
            if ((c = in(ctx)) < 0) return -1;
            dst[o++] = (uint8_t)c;
        }
        if (o == raw) break;                    /* last sequence: literals only */
        if ((lo = in(ctx)) < 0 || (hi = in(ctx)) < 0) return -1;
        off = (uint32_t)lo | ((uint32_t)hi << 8);
        ml = (uint32_t)tok & 15u;
        if (length(&ml, in, ctx)) return -1;
        ml += 4;
        if (off == 0 || off > o || ml > raw - o) return -1;
        while (ml--) {                          /* byte by byte: overlapping matches are legal */
            dst[o] = dst[o - off];
            o++;
        }
    }
    return 0;
}
