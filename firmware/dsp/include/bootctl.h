/* Boot-chain integration (boot/docs/RESCUE.md "Changes needed in the DSP firmware", LAYOUT.md):
 *   UPD  only if 0x0100 holds a valid "OCI1" header (never on the stock EEPROM): invalidate it (128 zero
 *        bytes at 0x0100, read back), then reboot: boot1 finds no
 *        header and stays in rescue mode (reason 3) without relying on RAM retention;
 *   RSC  software rescue request: RAM 0x1002DFF0 = 0x5352434F ("OCRS"), 0x1002DFF4 = its complement, then
 *        reboot (boot1/src/request.c; depends on RAM retention across the RTI reset);
 *   trial confirmation: for an image whose header has HF_TRIAL, write the "OCBS" record with confirmed =
 *        0xA5 once the TAS link is up and audio has run for 10 s (boot1/src/trial.c counts the attempts).
 * All EEPROM accesses go through eeprom.c (page policy there) and never overlap a profile job. */
#ifndef BOOTCTL_H
#define BOOTCTL_H

#include <stdint.h>

/* Trial status (BST "T:") */
enum { BT_WAIT, BT_NO_HEADER, BT_NOT_TRIAL, BT_CONFIRMED, BT_WROTE, BT_SKIPPED_RAM, BT_ERROR };
/* Last job result (BST "R:") */
enum { BR_NONE, BR_BUSY, BR_OK, BR_IO, BR_VERIFY, BR_REFUSED };

void bootctl_init(int cold);       /* cold = started by boot1 / ROM, not by the RAM loader */
void bootctl_poll(void);           /* main loop */
int  bootctl_upd(void);            /* 0 = started, -1 = busy */
void bootctl_rsc(void);
int  console_boot(const uint8_t *msg, int len, uint8_t *out, int max);  /* UPD, RSC, BST; -1 = not ours */

#endif
