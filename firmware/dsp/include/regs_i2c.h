/* TMS320C6000 I2C module (SPRU175D, "Registers" chapter). */
#ifndef REGS_I2C_H
#define REGS_I2C_H

#include "regs.h"

#define I2C_ICOAR   0x00   /* own address */
#define I2C_ICIMR   0x04   /* interrupt mask */
#define I2C_ICSTR   0x08   /* status */
#define I2C_ICCLKL  0x0C
#define I2C_ICCLKH  0x10
#define I2C_ICCNT   0x14
#define I2C_ICDRR   0x18   /* receive data */
#define I2C_ICSAR   0x1C
#define I2C_ICDXR   0x20   /* transmit data */
#define I2C_ICMDR   0x24   /* mode */
#define I2C_ICIVR   0x28   /* interrupt vector */
#define I2C_ICEMDR  0x2C
#define I2C_ICPSC   0x30   /* prescaler */

#define I2C_REG(base, off) REG32((base) + (off))

/* ICIMR / ICSTR bits */
#define I2C_AL     (1u << 0)
#define I2C_NACK   (1u << 1)
#define I2C_ARDY   (1u << 2)
#define I2C_ICRRDY (1u << 3)
#define I2C_ICXRDY (1u << 4)
#define I2C_SCD    (1u << 5)   /* stop condition detected */
#define I2C_AAS_IE (1u << 6)   /* ICIMR: "addressed as slave" interrupt enable */
#define I2C_AAS    (1u << 9)   /* ICSTR: addressed as slave */
#define I2C_SDIR   (1u << 14)  /* ICSTR: slave is transmitting */

/* ICIVR interrupt codes */
#define I2C_IV_NONE 0
#define I2C_IV_AL   1
#define I2C_IV_NACK 2
#define I2C_IV_ARDY 3
#define I2C_IV_RRDY 4
#define I2C_IV_XRDY 5
#define I2C_IV_SCD  6
#define I2C_IV_AAS  7

/* ICMDR bits */
#define I2C_MDR_IRS   (1u << 5)   /* module out of reset */
#define I2C_MDR_TRX   (1u << 9)
#define I2C_MDR_MST   (1u << 10)
#define I2C_MDR_FREE  (1u << 14)

#endif
