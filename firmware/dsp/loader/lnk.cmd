/* RAM loader: code, data and stack all fit in 0x10023000..0x10024DFF (7.5 KB).
   This is the tail of the stock firmware's .cinit table (0x1001DB40..0x10024E3F), which is only read by
   c_int00 at boot: after boot nothing references it (checked statically), so it can be overwritten.
   The end of RAM (0x1002D000..0x1002FFFF) is NOT free at runtime (console context at 0x1002DBA0, etc.). */
-e stub_entry

MEMORY
{
    STUB  : o = 0x10023000, l = 0x00001800
    STACK : o = 0x10024800, l = 0x00000600
}

SECTIONS
{
    .text:entry > STUB
    .text       > STUB
    .const      > STUB
    .switch     > STUB
    .data       > STUB
    .bss        > STUB
    .far        > STUB
    .fardata    > STUB
    .neardata   > STUB
    .rodata     > STUB
    .cinit      > STUB
}

__stack_top = 0x10024DF8;
