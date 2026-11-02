/* Rescue link: RAM loader protocol (../../README.md, RAM loader) carried by the TAS 'E' relay, shared by boot1's
 * rescue mode and by the RAM-only updater (which adds the EEPROM commands). Protocol: ../docs/RESCUE.md. */
#ifndef BOOT_RESCUE_H
#define BOOT_RESCUE_H

#include <stdint.h>

#define ACK 0x06
#define NAK 0x15

/* Sub-commands (first byte of the 'E' payload). 0x00..0x06 = RAM loader protocol, unchanged. */
enum {
    RC_INFO = 0x00, RC_WRITE = 0x01, RC_CRC = 0x02, RC_JUMP = 0x03, RC_READ = 0x04, RC_FILL = 0x05,
    RC_REBOOT = 0x06,
    RC_EE_READ = 0x10, RC_EE_WRITE = 0x11, RC_EE_CRC = 0x12,    /* updater only */
    RC_EE_UNLOCK_BOOT = 0x13, RC_EE_WRITE_BOOT0 = 0x14,         /* updater only, migration */
    RC_RESCUE = 0x7E                                            /* 7E 'O' 'C' 'R': stay in rescue */
};

#define MODE_RESCUE  'R'
#define MODE_UPDATER 'U'

extern int g_in_window;         /* boot window: only P, S and RC_RESCUE are served */
extern int g_rescue_requested;  /* set by RC_RESCUE */
extern const uint8_t g_mode;

void dspint_ready(void);
void dspint_release(void);
int  boot_window(void);         /* returns 1 if the TAS asked for rescue */

/* Extra 'E' sub-commands of the build (updater: EEPROM). Writes the reply into out (out[0] = ACK/NAK) and
 * returns its length, or 0 if the sub-command is unknown. */
unsigned cmd_ext(const uint8_t *msg, unsigned len, uint8_t *out);

#endif
