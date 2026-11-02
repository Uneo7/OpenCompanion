/* Host test of boot1's image path: the real image.c, stream.c, lz4.c, crc32.c, trial.c and util.c, compiled
 * for 32-bit x86 Windows, with the DSP RAM window mapped at its real address (0x10000000) and the EEPROM
 * replaced by a 64 KB file. Build and run: test/run_host_tests.sh.
 *
 *   host_image eeprom.bin ram_out.bin   -> boots the image like boot1, dumps [0x10000000, 0x1002AF00)
 *   host_image eeprom.bin --mutate      -> corrupts the image in many ways, every case must end in rescue */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>

#include "boot.h"
#include "ee.h"
#include "image.h"
#include "trial.h"
#include "layout.h"

static uint8_t ee[EE_SIZE];

/* EEPROM driver replaced by the file contents (same contract as boot1/src/ee.c). */
int ee_read(uint32_t addr, uint8_t *dst, uint32_t n)
{
    if (addr >= EE_SIZE || n > EE_SIZE - addr) return EE_ERR_RANGE;
    memcpy(dst, ee + addr, n);
    return EE_OK;
}

int ee_write(uint32_t addr, const uint8_t *src, uint32_t n)
{
    if (n == 0 || n > EE_PAGE || addr / EE_PAGE != (addr + n - 1u) / EE_PAGE) return EE_ERR_RANGE;
    if (addr < EE_WR_LO || addr + n > EE_WR_HI) return EE_ERR_RANGE;
    memcpy(ee + addr, src, n);
    return EE_OK;
}

static int boot(uint32_t *entry)
{
    img_hdr_t h;
    int r = image_read_header(&h);
    if (r == R_NONE) r = image_load(&h);
    if (r == R_NONE && (h.flags & HF_TRIAL)) r = trial_check(&h);
    *entry = h.entry;
    return r;
}

static int mutate(const uint8_t *orig)
{
    /* Offsets relative to EE_BODY unless noted; every case must give a rescue reason (r != R_NONE). */
    static const struct { const char *name; uint32_t addr; uint8_t xor_; } cases[] = {
        { "header magic", EE_HDR + 0, 0x01 }, { "header seq", EE_HDR + 8, 0x01 },
        { "header length", EE_HDR + 12, 0x04 }, { "header CRC", EE_HDR + 44, 0x80 },
        { "body magic", EE_BODY + 0, 0x01 }, { "first opcode", EE_BODY + 4, 0x01 },
        { "first address", EE_BODY + 11, 0x10 }, { "first raw size", EE_BODY + 12, 0x01 },
        { "first comp size", EE_BODY + 16, 0x01 }, { "first LZ4 byte", EE_BODY + 20, 0x01 },
        { "body byte 1000", EE_BODY + 1000, 0x01 }, { "body byte 20000", EE_BODY + 20000, 0x40 },
    };
    unsigned i, fails = 0;
    uint32_t entry, len = (uint32_t)orig[EE_HDR + 12] | ((uint32_t)orig[EE_HDR + 13] << 8);
    for (i = 0; i < sizeof cases / sizeof cases[0]; i++) {
        int r;
        memcpy(ee, orig, EE_SIZE);
        ee[cases[i].addr] ^= cases[i].xor_;
        r = boot(&entry);
        printf("%-18s -> reason %d %s\n", cases[i].name, r, r ? "(rescue, OK)" : "(BOOTS: FAIL)");
        fails += r == R_NONE;
    }
    /* Interrupted update: header written, body only partly written (rest erased). */
    for (i = 1; i <= 4; i++) {
        uint32_t cut = EE_BODY + len * i / 5;
        int r;
        memcpy(ee, orig, EE_SIZE);
        memset(ee + cut, 0xFF, EE_BODY_END - cut);
        r = boot(&entry);
        printf("body cut at %3u%%   -> reason %d %s\n", i * 20, r, r ? "(rescue, OK)" : "(BOOTS: FAIL)");
        fails += r == R_NONE;
    }
    /* Interrupted update: header invalidated (zeros), body anything. */
    memcpy(ee, orig, EE_SIZE);
    memset(ee + EE_HDR, 0, EE_PAGE);
    i = boot(&entry);
    printf("header zeroed      -> reason %u %s\n", i, i ? "(rescue, OK)" : "(BOOTS: FAIL)");
    fails += i == R_NONE;
    /* Trial: force the flag (header rebuilt by the test script), checked separately. */
    return fails ? 1 : 0;
}

int main(int argc, char **argv)
{
    FILE *f;
    uint8_t *ram, *orig;
    uint32_t entry;
    int r;
    if (argc < 3) { fprintf(stderr, "usage: host_image eeprom.bin (ram_out.bin | --mutate | --trial)\n"); return 2; }
    f = fopen(argv[1], "rb");
    if (!f || fread(ee, 1, EE_SIZE, f) != EE_SIZE) { fprintf(stderr, "cannot read %s\n", argv[1]); return 2; }
    fclose(f);
    ram = VirtualAlloc((void *)RAM_LO, RAM_HI - RAM_LO, MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE);
    if (ram != (uint8_t *)RAM_LO) { fprintf(stderr, "cannot map the DSP RAM window at 0x%08X\n", RAM_LO); return 2; }
    memset(ram, 0xA5, RAM_HI - RAM_LO);         /* poison: anything boot1 does not load stays 0xA5 */
    if (!strcmp(argv[2], "--mutate")) {
        orig = malloc(EE_SIZE);
        memcpy(orig, ee, EE_SIZE);
        return mutate(orig);
    }
    if (!strcmp(argv[2], "--trial")) {         /* header has HF_TRIAL: 2 starts, then rescue */
        int a = boot(&entry), b = boot(&entry), c = boot(&entry);
        printf("trial starts: %d %d %d (expected 0 0 %d)\n", a, b, c, R_TRIAL_EXHAUSTED);
        return !(a == R_NONE && b == R_NONE && c == R_TRIAL_EXHAUSTED);
    }
    r = boot(&entry);
    printf("boot1 image path: reason %d, entry 0x%08X\n", r, entry);
    f = fopen(argv[2], "wb");
    fwrite(ram, 1, APP_RAM_HI - APP_RAM_LO, f);
    fclose(f);
    return r != R_NONE;
}
