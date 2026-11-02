/* I2C0 slave 0x29, TAS 8-byte frame protocol, polled (no interrupt, no dMAX).
 * Same framing as the RAM loader (../../loader/loader.c, validated on hardware through the TAS 'E' relay). */
#ifndef BOOT_LINK_H
#define BOOT_LINK_H

#include <stdint.h>

#define LINK_MAX_MSG    64      /* the TAS relays at most 61 bytes */
#define LINK_MAX_RESP   61      /* above this the TAS answers "ERROR4" */

void link_init(void);
void link_poll(void);               /* one polling step */
int  link_bus_free(void);           /* I2C0 bus not busy (no START seen without its STOP) */
int  link_idle(void);               /* no transaction in progress and no reply left to read */
void link_shutdown(void);           /* I2C0 back in reset (pins released) */
void link_reply(uint8_t cmd, const uint8_t *d, unsigned n);

/* Provided by the command layer: */
void link_on_message(uint8_t cmd, const uint8_t *msg, unsigned len);  /* must call link_reply() */
void link_on_reply_read(void);      /* the TAS has read the last frame of a reply */

#endif
