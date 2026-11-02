/* OpenCompanion DSP linker command file (TMS320DA705, 192 KB internal RAM 0x10000000..0x1002FFFF).
 * - 0x10000000..0x10000FFF: not used. 0x10000400..0x10000FFF is reserved by the on-chip bootloader while
 *   it loads the AIS image (SPRAA69D), so no section may be loaded there.
 * - 0x10023000..0x10024FFF: reserved for the RAM loader (test images loaded without flashing). It lives in
 *   the tail of the stock .cinit table, the only area that is dead in the running Bose firmware. The loader's
 *   handoff record is at 0x10024E00 (include/handoff.h).
 * - 0x1002AF00..0x1002FFFF: boot chain (boot0 trampoline, boot1, rescue flag 0x1002DFF0, boot1 variables and
 *   stack; boot/docs/LAYOUT.md "RAM map"). Nothing may be LOADED there; IRAM_HI stops at 0x1002AEFF.
 * Linked with --ram_model and our own start-up (src/hw/reset.asm, start.c): initialised data is loaded in place,
 * .bss and .far are zeroed by oc_start(). Sections are grouped so the ELF has few segments: each segment is one
 * LZ4 section of the EEPROM body (16-byte header, separate LZ4 block). */
-heap  0x0
-stack 0x2000

MEMORY
{
    BOOT_RSVD : origin = 0x10000000, length = 0x00001000
    IRAM      : origin = 0x10001000, length = 0x00022000
    LOADER    : origin = 0x10023000, length = 0x00002000
    IRAM_HI   : origin = 0x10025000, length = 0x00005F00     /* up to 0x1002AEFF */
}

SECTIONS
{
    GROUP (LOADED) > IRAM
    {
        .vecs     : align(1024)
        .text
        .const
        .switch
        .rodata
        .neardata
        .fardata
    }
    GROUP (ZERO_INIT) > IRAM, RUN_START(__oc_zero_start), RUN_END(__oc_zero_end)
    {
        .bss      : align(8), type = NOLOAD
        .far      : palign(8), type = NOLOAD
    }
    .stack    > IRAM, type = NOLOAD, RUN_END(__oc_stack_end)        /* -stack size; SP set by reset.asm */
    .dsp_ram  > IRAM_HI, type = NOLOAD     /* block scratch buffers (kern.h DSP_SCRATCH): uninitialised, written before use */
    .bose_ram > IRAM_HI, type = NOLOAD     /* stock graph engine work RAM (src/bose/): uninitialised, not in the image,
                               cleared by bose_graph_init() */
    /* Stock graph program tables (src/bose/bose_graph_data.c: ops, wire lists, delays, outputs), linked in RAM
     * but NOT part of the EEPROM image body: oc-bootimg writes them to the graph data pages of the profile area
     * (boot/docs/LAYOUT.md, layout v3) and bose_tables.c copies them back at start, checked against BG_TAB_CRC
     * (generated with them). The RAM loader loads them with the rest of the ELF. Must stay the last
     * initialised section of IRAM_HI and a segment of its own. */
    .bose_tab > IRAM_HI, RUN_START(__oc_bose_tab_start), SIZE(__oc_bose_tab_size)
    {
        bose_graph_data.obj (.const:bg_ops)
        bose_graph_data.obj (.const:.string:bg_io)
        bose_graph_data.obj (.const:bg_dl)
        bose_graph_data.obj (.const:.string:bg_out)
    }
    .cinit    > IRAM
    .init_array > IRAM
    .sysmem   > IRAM
    .args     > IRAM
    .cio      > IRAM
}
