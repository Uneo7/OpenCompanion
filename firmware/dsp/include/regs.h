/* Generic register access + C672x / DA705 peripheral map.
 * Ref: docs/tms320c6727b.pdf (SPRS370E) "Memory Map" and "CPU Interrupt Assignments". */
#ifndef REGS_H
#define REGS_H

#include <stdint.h>

#define REG32(addr) (*(volatile uint32_t *)(addr))

/* Peripheral bases (SPRS370E memory map) */
#define PLL_BASE      0x41000000u
#define MCASP0_BASE   0x44000000u
#define MCASP1_BASE   0x45000000u
#define I2C0_BASE     0x49000000u
#define I2C1_BASE     0x4A000000u
#define DMAX_BASE     0x60000000u
#define HIMAX_PARAM   0x61008000u   /* HiMAX PaRAM: event table (words 0..31) then transfer entries */
#define LOMAX_PARAM   0x62008000u

/* McASP DMA data ports ("Word Only", any address in the range) */
#define MCASP0_DATA   0x54000000u
#define MCASP1_DATA   0x55000000u

/* CPU interrupt assignments (SPRS370E table "CPU Interrupt Assignments") */
#define INT_RTI0      4
#define INT_DMAX_FIFO 7
#define INT_DMAX_TCC  8    /* dMAX transfer completion (TCINT) */
#define INT_I2C_SPI   14   /* I2C0, I2C1, SPI0, SPI1 */

#endif
