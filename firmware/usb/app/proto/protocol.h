#ifndef PROTOCOL_H
#define PROTOCOL_H

#include <stdint.h>

/* Report layout (64 bytes, no report ID) */
#define RQ_CMD        0
#define RQ_TAG        1
#define RQ_LEN        2
#define RQ_DATA       3
#define RQ_MAX        61
#define RP_CMD        0
#define RP_TAG        1
#define RP_STATUS     2
#define RP_LEN        3
#define RP_DATA       4
#define RP_MAX        60

/* Status codes */
#define ST_OK         0x00
#define ST_BAD_LEN    0x01
#define ST_UNKNOWN    0x02
#define ST_BAD_ARG    0x03
#define ST_STATE      0x04      /* not allowed in the current state (e.g. no update session) */
#define ST_BAD_KEY    0x05      /* missing or wrong confirmation key */
/* 0x10..0x14: DSP link errors (board/dsp_link.h), 0x20..0x2F: EEPROM errors (proto/eeprom.h) */

#define ACTION_NONE   0
#define ACTION_MODE   0x30          /* = command code MODE (set): USB re-enumeration (main.c) */
#define ACTION_DFU    0x7E          /* = command code REBOOT_DFU (protocol.c dispatch) */
#define ACTION_REBOOT 0x7F          /* = command code REBOOT */

extern __data uint8_t protocol_action;     /* executed by main() once the reply has been collected */

void protocol_poll(void);                  /* handles one pending OUT report, if any */

#endif /* PROTOCOL_H */
