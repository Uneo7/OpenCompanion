/* Rescue command layer: TAS messages and the 'E' sub-commands of the RAM loader protocol
 * (../../../README.md, RAM loader: INFO, WRITE, CRC, JUMP, READ, FILL, REBOOT), so the existing host loader
 * (app/crates/oc-ramload) works unchanged against rescue mode. The RAM area of the boot
 * chain [RAM_BOOT_LO, RAM_HI) is never written, filled or jumped into. Other sub-commands go to cmd_ext()
 * (EEPROM commands in the updater build). INFO = 06 'O' 'C' 'L' 1 mode reason boot1-version. */
#include "boot.h"
#include "rescue.h"
#include "link.h"
#include "crc32.h"
#include "layout.h"

#define LOADER_PROTO 1          /* same protocol version as the RAM loader */

static uint32_t jump_to;
static int reboot_pending, reboot_ready;

/* Polls of an idle I2C0 bus required before the reboot (~ms): the TAS must have finished its transaction
 * and released the bus, otherwise a DSP reset in the middle of its next access can hang the stock TAS. */
#define REBOOT_IDLE_POLLS 200000u

static int in_range(uint32_t a, uint32_t n, uint32_t lo, uint32_t hi)
{
    return a >= lo && n <= hi - lo && a - lo <= (hi - lo) - n;
}

static unsigned ack(uint8_t *out) { out[0] = ACK; return 1; }

static unsigned ram_cmd(const uint8_t *m, unsigned len, uint8_t *out)
{
    uint32_t a = len >= 5 ? rd32(m + 1) : 0, n, i;
    switch (m[0]) {
    case RC_INFO:
        out[0] = ACK; out[1] = 'O'; out[2] = 'C'; out[3] = 'L'; out[4] = LOADER_PROTO;
        out[5] = g_mode; out[6] = g_boot_reason; out[7] = BOOT1_VERSION;
        return 8;
    case RC_WRITE:                              /* [01][addr][data...] */
        if (len < 5 || !in_range(a, n = len - 5, RAM_LO, RAM_BOOT_LO)) break;
        for (i = 0; i < n; i++) ((volatile uint8_t *)a)[i] = m[5 + i];
        return ack(out);
    case RC_FILL:                               /* [05][addr][len][byte] */
        if (len < 10 || !in_range(a, n = rd32(m + 5), RAM_LO, RAM_BOOT_LO)) break;
        for (i = 0; i < n; i++) ((volatile uint8_t *)a)[i] = m[9];
        return ack(out);
    case RC_CRC:                                /* [02][addr][len] -> [06][crc32] */
        if (len < 9 || (n = rd32(m + 5)) > 0x10000u || !in_range(a, n, RAM_LO, RAM_HI)) break;
        out[0] = ACK;
        wr32(out + 1, crc32((const volatile uint8_t *)a, n));
        return 5;
#ifdef BOOT_UPDATER
    case RC_READ:                               /* [04][addr][n] -> [06][n bytes] (updater only: size) */
        if (len < 6 || (n = m[5]) > LINK_MAX_RESP - 1 || !in_range(a, n, RAM_LO, RAM_HI)) break;
        out[0] = ACK;
        for (i = 0; i < n; i++) out[1 + i] = ((const volatile uint8_t *)a)[i];
        return n + 1;
#endif
    case RC_JUMP:                               /* [03][addr], jump once the reply has been read */
        if (len < 5 || !in_range(a, 4, RAM_LO, RAM_BOOT_LO)) break;
        jump_to = a;
        return ack(out);
    case RC_REBOOT:                             /* reboot once the reply has been read */
        reboot_pending = 1;
        return ack(out);
    default:
        return 0;
    }
    out[0] = NAK;
    return 1;
}

static void e_cmd(const uint8_t *m, unsigned len)
{
    uint8_t out[LINK_MAX_RESP];
    unsigned n = 0;
    if (len >= 4 && m[0] == RC_RESCUE && m[1] == 'O' && m[2] == 'C' && m[3] == 'R') {
        g_rescue_requested = 1;
        out[0] = ACK; out[1] = 'O'; out[2] = 'C'; out[3] = 'R';
        n = 4;
    } else if (len >= 1 && !g_in_window) {
        n = ram_cmd(m, len, out);
        if (!n) n = cmd_ext(m, len, out);
    }
    if (!n) { out[0] = NAK; n = 1; }
    link_reply('E', out, n);
}

void link_on_message(uint8_t cmd, const uint8_t *msg, unsigned len)
{
    static const uint8_t alive[5] = { 'A', 'L', 'I', 'V', 'E' };
    static const uint8_t status[2] = { 0, 0 };  /* byte 1 = 0: never request standby */
    switch (cmd) {
    case 'E': e_cmd(msg, len); break;
    case 'P': link_reply(cmd, alive, 5); break;
    case 'S': link_reply(cmd, status, 2); break;
    case 'U':
        if (!g_in_window) dspint_release();
        link_reply(cmd, 0, 0);
        break;
    default: link_reply(cmd, 0, 0); break;     /* X, F, V, m, M, D, d...: accepted and ignored */
    }
}

/* Not during the boot window: jump_to / reboot_pending are only initialised by rescue_run(), and boot1 has no
 * C runtime (.bss not zeroed, LFLAGS --zero_init=off). On a cold boot they hold random RAM, so a TAS reading
 * any reply frame in the window (e.g. an 'S' poll) used to jump to a garbage address. */
void link_on_reply_read(void)
{
    if (g_in_window) return;
    if (reboot_pending) reboot_ready = 1;      /* rebooted from rescue_run once the bus is idle */
    if (jump_to) start_app(jump_to);
}

void rescue_run(void)
{
    unsigned idle = 0;
    jump_to = 0;
    reboot_pending = 0;
    reboot_ready = 0;
    if (!g_rescue_requested)
        link_init();                            /* else: keep the link opened by the boot window */
    dspint_ready();
    for (;;) {
        link_poll();
        if (reboot_ready) {
            if (link_idle() && link_bus_free()) {
                if (++idle > REBOOT_IDLE_POLLS) reboot_to_rom();
            } else {
                idle = 0;
            }
        }
    }
}
