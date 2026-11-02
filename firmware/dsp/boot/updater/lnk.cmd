/* Updater (RAM only): code at 0x10001000 (where the firmware itself starts), variables and stack above it.
 * Outside the RAM loader area (0x10023000..0x10024DFF) and the boot chain area (>= 0x1002AF00).
 * 0x10020000..0x100200FF is used by the host to stage boot0 during the migration. */
-e updater_entry

MEMORY
{
    CODE  : o = 0x10001000, l = 0x00003000
    DATA  : o = 0x10004000, l = 0x00001800
    STACK : o = 0x10005800, l = 0x00000800
}

SECTIONS
{
    .text:entry : load = 0x10001000
    .text       > CODE
    .const      > CODE
    .switch     > CODE
    .fardata    > CODE
    .neardata   > CODE
    .rodata     > CODE
    .cinit      > CODE
    .far        > DATA
    .bss        > DATA
}

__stack_top = 0x10005FF8;
