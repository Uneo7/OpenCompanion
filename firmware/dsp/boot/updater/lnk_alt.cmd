/* Updater, alternate link: same program at 0x10008000, for loading a new updater while another one runs
 * (the running one keeps 0x10001000..0x10005FFF until the JUMP). Only ever loaded in RAM by the host.
 * Free of: the default updater (0x10001000..0x10005FFF), the RAM loader area (0x10023000..0x10024DFF),
 * the boot0 staging area (0x10020000..0x100200FF) and the boot chain area (>= 0x1002AF00).
 * 0x10008000..0x1000C7FF only holds stock firmware state, dead once the RAM loader has stopped it. */
-e updater_entry

MEMORY
{
    CODE  : o = 0x10008000, l = 0x00003000
    DATA  : o = 0x1000B000, l = 0x00001000
    STACK : o = 0x1000C000, l = 0x00000800
}

SECTIONS
{
    .text:entry : load = 0x10008000
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

__stack_top = 0x1000C7F8;
