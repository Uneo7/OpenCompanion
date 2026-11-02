/* Minimal console reached through the TAS 'E' command.
 * Same conventions as the Bose firmware (re/dsp/notes/control.md):
 *  - 3-letter command;
 *  - binary commands (BRR, BWW): little-endian arguments, reply 0x06 (ACK) or 0x15 (NAK);
 *  - text commands (RST, SVM, SYS, LVI, LVO, UPD, RSC, BST, and the input
 *    commands of console_inputs.c) must end with '\r';
 *    text replies end with "\r>", errors are "\r!". Replies stay under 58 bytes (TAS relay limit).
 * BRR <n:LE32 1..4> <addr:LE32>               -> 06 + 4n bytes
 * BWW <n:LE32 1..12> <addr:LE32> <4n bytes>   -> 06 / 15 (writes restricted to internal RAM)
 * OP* : sound parameters and stored profiles, see console_params.c; DEV: device settings, console_dev.c */
#include <string.h>
#include "oc.h"
#include "prof.h"
#ifdef OC_PROF
#include <c6x.h>
#endif
#include "inputs.h"
#include "bootctl.h"
#include "console_util.h"
#include "rate.h"

#ifndef OC_STAGE
#define OC_STAGE 4
#endif

#define ACK 0x06
#define NAK 0x15
#define RAM_START 0x10000000u
#define RAM_END   0x10030000u

const char *i2c_tas_version(void);
int console_params(const uint8_t *msg, int len, uint8_t *out, int max);
int console_dev(const uint8_t *msg, int len, uint8_t *out, int max);

static uint32_t le32(const uint8_t *p)
{
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

static int end_ok(uint8_t *out, int n, int max) { return cu_str(out, n, max, "\r>"); }
static int err(uint8_t *out, int max) { return cu_str(out, 0, max, "\r!"); }

static long meter_db(int idx, int reset)
{
    return meters_to_db(meters_peak(idx, reset));
}

static int put_levels(uint8_t *out, int max, int first, int count)
{
    int i, n = 0;
    for (i = 0; i < count; i++) {
        if (i) n = cu_str(out, n, max, ",");
        n = cu_int(out, n, max, meter_db(first + i, 1));
    }
    return end_ok(out, n, max);
}

static long max_db(int first, int count)
{
    int i;
    long best = -99;
    for (i = 0; i < count; i++) {
        long db = meter_db(first + i, 0);
        if (db > best) best = db;
    }
    return best;
}

static int cmd_brr(const uint8_t *a, int len, uint8_t *out, int max)
{
    uint32_t n, addr, i;
    if (len != 8) { out[0] = NAK; return 1; }
    n = le32(a);
    addr = le32(a + 4);
    if (n < 1 || n > 4 || 1 + 4 * (int)n > max || (addr & 3u)) { out[0] = NAK; return 1; }
    out[0] = ACK;
    for (i = 0; i < n; i++) {
        uint32_t w = *(volatile uint32_t *)(addr + 4 * i);
        memcpy(out + 1 + 4 * i, &w, 4);
    }
    return 1 + 4 * (int)n;
}

static int cmd_bww(const uint8_t *a, int len, uint8_t *out)
{
    uint32_t n, addr, i;
    if (len < 12) { out[0] = NAK; return 1; }
    n = le32(a);
    addr = le32(a + 4);
    if (n < 1 || len != 8 + 4 * (int)n || (addr & 3u) ||
        addr < RAM_START || addr + 4 * n > RAM_END) { out[0] = NAK; return 1; }
    for (i = 0; i < n; i++)
        *(volatile uint32_t *)(addr + 4 * i) = le32(a + 8 + 4 * i);
    out[0] = ACK;
    return 1;
}

static int text_fields(const uint8_t *msg, uint8_t *out, int max)
{
    static const char L_SYS[] = "FR:\0\rCL:\0\rVOL:\0\rMU:\0\rPK:\0/";
    long v[8];
    /* CPU: measured load (audio_io.c): average and maximum % of the frame period, period in RTI ticks, lost frames */
#ifdef OC_PROF
    /* PRF0 / PRF1: profiling build only (prof.h): "A:" stages 0..7, "B:" stages 8..15, in 0.1 % of the frame period */
    if (!memcmp(msg, "PRF", 3)) {
        static const char L_A[] = "A:\0 \0 \0 \0 \0 \0 \0 ", L_B[] = "B:\0 \0 \0 \0 \0 \0 \0 ";
        const int first = msg[3] == '1' ? 8 : 0;
        if (msg[3] == '2') {                   /* PRF2: program cache mode CSR.PCC (2 = enabled) */
            static const char L_PC[] = "PC:";
            v[0] = (long)((CSR >> 5) & 7u);
            return end_ok(out, cu_fields(out, 0, max, L_PC, v, 1), max);
        }
        const float per = g_cpu.period ? 1000.0f / (float)g_cpu.period : 0.0f;
        int k;
        for (k = 0; k < 8; k++) v[k] = cu_fx((float)g_prof[first + k] * per, 1.0f);
        return end_ok(out, cu_fields(out, 0, max, first ? L_B : L_A, v, 8), max);
    }
#endif
    if (!memcmp(msg, "CPU", 3)) {
        static const char L_CPU[] = "LD:\0 MX:\0 T:\0 LF:";
        const float per = g_cpu.period ? (float)g_cpu.period : 1.0f;
        v[0] = cu_fx((float)g_cpu.avg / per, 100.0f);
        v[1] = cu_fx((float)g_cpu.max / per, 100.0f);
        v[2] = (long)g_cpu.period;
        v[3] = (long)g_cpu.lost;
        g_cpu.max = 0;
        return end_ok(out, cu_fields(out, 0, max, L_CPU, v, 4), max);
    }
    if (!memcmp(msg, "SYS", 3)) {
        v[0] = (long)g_audio.frames;
        v[1] = (long)g_audio.clips;
        v[2] = (long)g_audio.volume_db;
        v[3] = (long)(g_audio.usb_mute | (g_audio.pod_mute << 1));
        v[4] = max_db(0, METER_IN_COUNT);                      /* max input / output peak, dBFS */
        v[5] = max_db(METER_OUT0, OUT_COUNT);
        return end_ok(out, cu_fields(out, 0, max, L_SYS, v, 6), max);
    }
    return -1;
}

int console_exec(const uint8_t *msg, int len, uint8_t *out, int max)
{
    int n;
    if (len < 3 || max < 8) return err(out, max);

    if (!memcmp(msg, "BRR", 3)) return cmd_brr(msg + 3, len - 3, out, max);
    if (!memcmp(msg, "BWW", 3)) return cmd_bww(msg + 3, len - 3, out);
#if OC_STAGE >= 3
    n = console_params(msg, len, out, max);
    if (n >= 0) return n;
#endif

    /* Text commands: must end with '\r' */
    if (msg[len - 1] != '\r') return err(out, max);
#if OC_STAGE >= 4
    n = console_inputs(msg, len, out, max);   /* POD, BAS, VRW, RMP, GPI, STB, PRT (console_inputs.c) */
    if (n >= 0) return n;
    n = console_dev(msg, len, out, max);      /* DEV: device settings (console_dev.c) */
    if (n >= 0) return n;
#endif
    n = console_boot(msg, len, out, max);     /* UPD, RSC, BST (bootctl.c) */
    if (n >= 0) return n;
    n = text_fields(msg, out, max);           /* SYS, CPU */
    if (n >= 0) return n;
    if (!memcmp(msg, "RST", 3)) {             /* reboot to the ROM boot loader (EEPROM image) */
        g_reboot_request = 1;
        return end_ok(out, 0, max);
    }
    if (!memcmp(msg, "SVM", 3)) {
        n = cu_str(out, 0, max, "D" OC_VERSION "U");
        n = cu_str(out, n, max, i2c_tas_version());
        return end_ok(out, n, max);
    }
    /* Level meters (integer dBFS, -99 = silence):
     *  LVI / LVO : peak hold of the 8 inputs / 6 outputs since the previous read (then reset). */
    if (msg[0] == 'L' && msg[1] == 'V' && (msg[2] == 'I' || msg[2] == 'O')) {
        int outputs = msg[2] == 'O';
        return put_levels(out, max, outputs ? METER_OUT0 : 0, outputs ? OUT_COUNT : METER_IN_COUNT);
    }
    return err(out, max);
}
