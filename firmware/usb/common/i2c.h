#ifndef I2C_H
#define I2C_H

#include <stdint.h>

#define I2C_SUBADDR 0x80    /* OR into `dev`: send a 2-byte sub-address (big-endian) before the data */

void i2c_init(void);
/* Return 1 on success, 0 on NACK / timeout. len >= 1. */
uint8_t i2c_write(uint8_t dev, uint16_t sub, const __xdata uint8_t *data, uint8_t len);
uint8_t i2c_read(uint8_t dev, uint16_t sub, __xdata uint8_t *data, uint8_t len);

#endif /* I2C_H */
