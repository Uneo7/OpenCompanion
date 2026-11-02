/* Graph program tables from the EEPROM graph data pages, see include/bose_tables.h.
 * .bose_tab holds bg_ops, bg_io, bg_dl, bg_out back to back with no padding (lnk.cmd order; 528 + 356 + 32 + 5
 * bytes), so its CRC-32 is the one gen_bose_graph.py computes over the same tables (BG_TAB_CRC). Any other
 * layout or content fails the check and the engine stays off (safe). */
#include "bose_tables.h"
#include "bose_graph.h"
#include "eeprom.h"
#include "crc32.h"

extern far uint8_t __oc_bose_tab_start[], __oc_bose_tab_size[];   /* lnk.cmd .bose_tab */

static int source = BT_MISSING;

static int tables_match(void)
{
    return oc_crc32(__oc_bose_tab_start, (uint32_t)__oc_bose_tab_size) == BG_TAB_CRC;
}

void bose_tables_init(void)
{
    uint8_t h[BT_HDR_SIZE];
    uint8_t *dst = __oc_bose_tab_start;
    uint32_t k, n, left = (uint32_t)__oc_bose_tab_size;
    source = BT_RAM;
    if (tables_match()) return;                         /* loaded with the ELF (RAM loader) */
    eeprom_init();
    source = BT_MISSING;
    if (!eeprom_read_wait((uint16_t)BT_PAGE(0), h, BT_HDR_SIZE) || oc_get32(h) != BT_MAGIC ||
        oc_get32(h + 4) != (BT_VERSION | (left << 16)) || oc_get32(h + 8) != (uint32_t)dst)
        return;
    for (k = 0; k < BT_PAGES && left; k++) {
        const uint32_t off = k ? 0u : BT_HDR_SIZE;      /* page 0 starts with the header */
        n = 128u - off;
        if (n > left) n = left;
        if (!eeprom_read_wait((uint16_t)(BT_PAGE(k) + off), dst, (uint16_t)n))
            return;
        dst += n;
        left -= n;
    }
    if (tables_match()) source = BT_EEPROM;
}

int bose_tables_ok(void) { return source != BT_MISSING; }

/* Page p = (addr - PROF_AREA_LO) / 128 of the 48 copy pages: graph data pages are p = 6k + 5 (bit masks). */
int bose_tables_is_data_page(uint32_t addr)
{
    static const uint32_t M[2] = { 0x20820820u, 0x00008208u };
    const uint32_t p = (addr - PROF_AREA_LO) >> 7;
    return addr >= PROF_AREA_LO && p < 48u && ((M[p >> 5] >> (p & 31u)) & 1u);
}
