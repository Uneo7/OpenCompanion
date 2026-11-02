#ifndef BOOT_H
#define BOOT_H

#include <stdint.h>

#define BOOT1_VERSION   1

/* Why boot1 is in rescue mode (reported by the rescue INFO command). */
enum {
    R_NONE = 0,
    R_SOFT_REQUEST,     /* RAM flag set by our firmware before a reboot */
    R_TAS_REQUEST,      /* 'E' RESCUE request received from the TAS during the boot window */
    R_NO_HEADER,        /* header page blank or not "OCI1" (also: after the stock image, during migration) */
    R_BAD_HEADER,       /* header CRC, version or length invalid */
    R_BAD_BODY,         /* body opcode, range or length invalid */
    R_BAD_CRC,          /* body CRC-32 mismatch */
    R_EE_IO,            /* I2C1 / EEPROM error */
    R_TRIAL_EXHAUSTED,  /* new image started BOOT_TRIALS times without being confirmed */
    R_UPDATER           /* the RAM-only updater is running */
};

extern uint8_t g_boot_reason;

uint32_t rd16(const uint8_t *p);
uint32_t rd32(const uint8_t *p);
void wr32(uint8_t *p, uint32_t v);

void pll_init(void);
void reboot_to_rom(void);           /* does not return */
void start_app(uint32_t entry);     /* does not return */

int  request_soft(void);            /* 1 if our firmware asked for rescue (flag consumed) */
void rescue_run(void);              /* does not return */

#endif
