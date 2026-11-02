#include "console_util.h"

int cu_str(uint8_t *o, int n, int max, const char *s)
{
    while (*s && n < max) o[n++] = (uint8_t)*s++;
    return n;
}

/* Decimal digits by subtracting powers of ten: no RTS division / remainder helpers in the image. */
int cu_int(uint8_t *o, int n, int max, long v)
{
    static const uint32_t P10[10] = { 1000000000u, 100000000u, 10000000u, 1000000u, 100000u, 10000u, 1000u,
                                      100u, 10u, 1u };
    char t[12];
    int i = 0, k;
    uint32_t u = v < 0 ? (uint32_t)-v : (uint32_t)v;
    if (v < 0) t[i++] = '-';
    for (k = 0; k < 10; k++) {
        char d = '0';
        while (u >= P10[k]) { u -= P10[k]; d++; }
        if (d != '0' || i > (v < 0) || k == 9) t[i++] = d;
    }
    t[i] = 0;
    return cu_str(o, n, max, t);
}

int cu_hex(uint8_t *o, int n, int max, uint32_t v)
{
    char t[9];
    int i;
    for (i = 0; i < 8; i++) t[i] = "0123456789ABCDEF"[(v >> (28 - 4 * i)) & 15u];
    t[8] = 0;
    return cu_str(o, n, max, t);
}

int cu_fields(uint8_t *o, int n, int max, const char *lab, const long *v, int cnt)
{
    int i;
    for (i = 0; i < cnt; i++) {
        n = cu_str(o, n, max, lab);
        while (*lab++) ;                    /* next label */
        n = cu_int(o, n, max, v[i]);
    }
    return n;
}

long cu_fx(float v, float scale) { return (long)(v * scale + (v < 0.0f ? -0.5f : 0.5f)); }

int cu_parse_int(const uint8_t *a, int len, long *v)
{
    int neg = len > 0 && a[0] == '-', i = neg;
    long x = 0;
    if (i >= len || len - i > 5) return 0;
    for (; i < len; i++) {
        if (a[i] < '0' || a[i] > '9') return 0;
        x = x * 10 + (a[i] - '0');
    }
    *v = neg ? -x : x;
    return 1;
}
