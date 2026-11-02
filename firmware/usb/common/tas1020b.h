/*
 * TAS1020B memory-mapped registers (external data space).
 * Source: TI Data Manual SLES025B ("DM" below), section 6.5 (register map) and 6.4 (endpoint blocks).
 * Only registers used by the OpenCompanion firmwares are listed.
 */
#ifndef TAS1020B_H
#define TAS1020B_H

#include <stdint.h>

#define XREG(addr) (*(volatile __xdata uint8_t *)(addr))

/* --- USB registers (DM 6.5.1) --- */
#define USBFADR   XREG(0xFFFF)   /* function address */
#define USBSTA    XREG(0xFFFE)   /* status: RSTR SUSR RESR SOF PSOF SETUP - STPOW */
#define USBIMSK   XREG(0xFFFD)   /* interrupt mask, same layout as USBSTA */
#define USBCTL    XREG(0xFFFC)   /* CONT FEN RWUP FRSTE - - - SDW_OK */
#define USBFNL    XREG(0xFFFB)   /* frame number, low byte */

#define USBSTA_RSTR   0x80
#define USBSTA_SUSR   0x40
#define USBSTA_RESR   0x20
#define USBSTA_SOF    0x10
#define USBSTA_PSOF   0x08
#define USBSTA_SETUP  0x04
#define USBSTA_STPOW  0x01

#define USBCTL_CONT   0x80       /* connect: drives the D+ pull-up (PUR) */
#define USBCTL_FEN    0x40       /* function enable, cleared by USB reset */
#define USBCTL_FRSTE  0x10
#define USBCTL_SDW_OK 0x01       /* must be set before clearing MEMCFG.SDW */

/* --- DMA registers (DM 6.5.2); only channel 0 is used --- */
#define DMABCNT0H XREG(0xFFEC)   /* ISO OUT buffer content in bytes, updated at each SOF (high) */
#define DMABCNT0L XREG(0xFFEB)   /* (low) */
#define DMATSL0   XREG(0xFFEA)   /* time slots 7..0 served by channel 0 */
#define DMATSH0   XREG(0xFFE9)   /* BPTS1 BPTS0 TSL13..TSL8 */
#define DMACTL0   XREG(0xFFE8)   /* DMAEN HSKEN - - EPDIR EPNUM2..0 */
#define DMATSH_BPTS(n) ((uint8_t)(((n) - 1) << 6))   /* bytes per time slot, n = 1..4 */
#define DMACTL_DMAEN   0x80
#define DMACTL_EPDIR   0x08      /* 1 = IN endpoint */

/* --- Adaptive clock generator (DM 2.2.6, 6.5.3) --- */
#define ACG1FRQ0  XREG(0xFFE7)   /* LSB; writing it loads the 24-bit value (DM 2.2.6.1) */
#define ACG1FRQ1  XREG(0xFFE6)
#define ACG1FRQ2  XREG(0xFFE5)   /* MSB */
#define ACGCAPL   XREG(0xFFE4)   /* MCLK cycles captured at each SOF (low) */
#define ACGCAPH   XREG(0xFFE3)   /* (high) */
#define ACG1DCTL  XREG(0xFFE2)   /* DIVM3..0 - DIVI2..0 */
#define ACGCTL    XREG(0xFFE1)   /* MCLKO2EN MCLKO1EN - MCLKO1S1 MCLKO1S0 DIVEN MCLKO2S1 MCLKO2S0 */
#define ACG2FRQ0  XREG(0xFFF9)
#define ACG2FRQ1  XREG(0xFFF8)
#define ACG2FRQ2  XREG(0xFFF7)
#define ACG2DCTL  XREG(0xFFF6)
#define ACGCTL_MCLKO2EN 0x80
#define ACGCTL_MCLKO1EN 0x40
#define ACGCTL_DIVEN    0x04
#define ACGCTL_MCLKO2S1 0x02     /* MCLKO2 source = ACG2 (after /M) */

/* --- Codec port (DM 2.2.13, 6.5.4); config registers are writable only while GLOBCTL.CPTEN = 0 --- */
#define CPTCNF1   XREG(0xFFE0)   /* NTSL4..0 MODE2..0 */
#define CPTCNF2   XREG(0xFFDF)   /* TSL0L1..0 BPTSL2..0 TSLL2..0 */
#define CPTCNF3   XREG(0xFFDE)   /* DDLY TRSEN CSCLKP CSYNCP CSYNCL BYOR CSCLKD CSYNCD */
#define CPTCNF4   XREG(0xFFDD)   /* ATSL3..0 CPTBLK DIVB2..0 */
#define CPTCNF2_BPTSL_16 (1 << 3)
#define CPTCNF2_BPTSL_24 (4 << 3)
#define CPTCNF2_BPTSL_32 (5 << 3)

/* --- I2C master (DM 2.2.14, 6.5.6) --- */
#define I2CADR    XREG(0xFFC3)   /* A6..A0 RW */
#define I2CDATI   XREG(0xFFC2)
#define I2CDATO   XREG(0xFFC1)
#define I2CCTL    XREG(0xFFC0)   /* RXF RXIE ERR FRQ TXE TXIE STPRD STPWR */
#define I2CCTL_RXF    0x80
#define I2CCTL_ERR    0x20
#define I2CCTL_FRQ    0x10       /* 1 = 400 kHz */
#define I2CCTL_TXE    0x08
#define I2CCTL_STPRD  0x02
#define I2CCTL_STPWR  0x01

/* --- Miscellaneous registers (DM 6.5.7) --- */
#define VECINT    XREG(0xFFB2)   /* read: pending vector; write any value: acknowledge */
#define GLOBCTL   XREG(0xFFB1)   /* MCUCLK XINTEN P1PUDIS VREN RESET LPWR P3PUDIS CPTEN */
#define MEMCFG    XREG(0xFFB0)   /* MEMTYP CODESZ1 CODESZ0 REV3..0 SDW */

#define GLOBCTL_MCUCLK 0x80      /* 24 MHz MCU clock (cannot be cleared except by master reset) */
#define GLOBCTL_VREN   0x10      /* VREN pin 42 (board: VRAWON) */
#define GLOBCTL_RESET  0x08      /* RESET pin 41 (board: DSPRESET\) */
#define GLOBCTL_LPWR   0x04      /* 1 = USB blocks powered (normal operation) */
#define GLOBCTL_CPTEN  0x01      /* codec port enable */
#define MEMCFG_SDW     0x01

/* VECINT values (DM 6.5.7.3) */
#define VEC_OUT_EP(n)  (0x00 + (n))
#define VEC_IN_EP(n)   (0x08 + (n))
#define VEC_STPOW      0x10
#define VEC_SETUP      0x12
#define VEC_PSOF       0x13
#define VEC_SOF        0x14
#define VEC_RESUME     0x15
#define VEC_SUSPEND    0x16
#define VEC_RESET      0x17
#define VEC_NONE       0x24

/* Endpoint configuration blocks (DM 6.4, table 6-1): OUT n at 0xFFA8 - 8n, IN n at 0xFF68 - 8n */
#define OEP_BASE(n)    (0xFFA8 - 8 * (n))
#define IEP_BASE(n)    (0xFF68 - 8 * (n))
#define EP_CNF(b)      XREG((b) + 0)  /* EN ISO TOGGLE DBUF STALL IE - -   (ISO: EN ISO OVF BPS4..0) */
#define EP_BBAX(b)     XREG((b) + 1)  /* X buffer base, (addr - 0xF800) / 8 */
#define EP_BSIZ(b)     XREG((b) + 2)  /* buffer size in 8-byte units (ISO: size of the circular buffer) */
#define EP_DCNTX(b)    XREG((b) + 3)  /* NACK | count */
#define EP_BBAY(b)     XREG((b) + 5)
#define EP_DCNTY(b)    XREG((b) + 7)

#define EPCNF_EN       0x80
#define EPCNF_ISO      0x40
#define EPCNF_TOGGLE   0x20
#define EPCNF_STALL    0x08
#define EPCNF_IE       0x04
#define EPDCNT_NACK    0x80
#define EPDCNT_MASK    0x7F

/* Buffer address bytes are offsets from 0xF800 (TUSB3200 family compatibility, DM 6.4.2) */
#define EP_BUF_ORIGIN  0xF800
#define EP_BUF_REG(a)  ((uint8_t)(((a) - EP_BUF_ORIGIN) >> 3))

/* Control endpoint setup packet buffer (DM 6.4.5) */
#define SETUP_BUF      ((volatile __xdata uint8_t *)0xFF28)

#endif /* TAS1020B_H */
