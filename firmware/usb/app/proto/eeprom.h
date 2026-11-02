#ifndef EEPROM_H
#define EEPROM_H

#include <stdint.h>

#define EE_ERR_STATE    0x04      /* no update session (same value as ST_STATE) */
#define EE_ERR_IO       0x20      /* I2C error */
#define EE_ERR_VERIFY   0x21      /* read-back differs */
#define EE_ERR_RANGE    0x22      /* address range not allowed */
#define EE_ERR_HEADER   0x23      /* header refused (current or proposed) */

extern __bit ee_session;           /* update session open (header DataType = 3) */

/* Data come from / go to the vendor HID buffers (config.h HID_REQ_DATA / HID_REP_DATA). */
uint8_t ee_read(uint16_t addr, uint8_t n);           /* -> reply payload */
uint8_t ee_begin(void);                              /* request payload = key; reply payload used as scratch */
uint8_t ee_write(uint16_t addr, uint8_t n);          /* request payload [addr H][addr L][n bytes] */
uint8_t ee_commit(void);                             /* request payload = 18-byte header */
void ee_end(void);
uint8_t ee_write_mode(void);                         /* request payload [key x 4][mode code] -> mode record */

#endif /* EEPROM_H */
