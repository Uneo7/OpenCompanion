/* Program tables of the "Bose d'origine" engine kept out of the EEPROM image body (EEPROM layout v3,
 * boot/docs/LAYOUT.md "Graph data pages").
 *
 * bg_ops, bg_io, bg_dl and bg_out (src/bose/bose_graph_data.c, 921 bytes) are linked in RAM in their own
 * section .bose_tab (lnk.cmd), which oc-bootimg leaves out of the image body and writes instead to the eight
 * graph data pages: the last 128-byte page of each profile copy, never used by a profile record (632 bytes in
 * a 0x300 copy) and never written by any firmware:
 *   page k (k = 0..7) at PROF_AREA_LO + k * PROF_COPY_SIZE + 0x280
 *   page 0: +0 magic "OCG1"  +4 version (LE16) = 1  +6 payload length (LE16)  +8 RAM address of the
 *           payload (LE32) = start of .bose_tab  +12 CRC-32 (zlib) of the payload  +16 payload bytes...
 *   pages 1..7: payload continued (128 bytes each); capacity 1008 bytes.
 * At start (bose_tables_init(), after the profile scan) the tables in RAM (CRC-32 of the whole section) are
 * checked against BG_TAB_CRC
 * (generated with them, include/bose_graph_size.h): a RAM-loaded ELF brings them with it. Otherwise they are
 * read from the data pages and checked again. If they are missing or do not match this firmware, the engine
 * never runs: a profile with PF_STOCK_GRAPH falls back to the neutral preset (its own bands: flat for the built-in
 * "Bose d'origine"), and OPI flags bit 3 tells the app. */
#ifndef BOSE_TABLES_H
#define BOSE_TABLES_H

#include <stdint.h>
#include "profiles.h"

#define BT_PAGES      8u
#define BT_PAGE_OFF   0x280u                                   /* last page of a profile copy */
#define BT_PAGE(k)    (PROF_AREA_LO + (k) * PROF_COPY_SIZE + BT_PAGE_OFF)
#define BT_HDR_SIZE   16u
#define BT_CAPACITY   (BT_PAGES * 128u - BT_HDR_SIZE)          /* 1008 bytes */
#define BT_MAGIC      0x3147434Fu                              /* "OCG1" little-endian */
#define BT_VERSION    1u

enum { BT_MISSING, BT_RAM, BT_EEPROM };

void bose_tables_init(void);          /* boot, blocking EEPROM reads (eeprom_init() done) */
int  bose_tables_ok(void);            /* the engine may run */
int  bose_tables_is_data_page(uint32_t addr);   /* addr inside a graph data page (EEPROM write policy) */

#endif
