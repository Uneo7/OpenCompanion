/* boot0 trampoline: 232 bytes of code at most (the assembler pads the section to 256 with NOPs; the host
 * tool loads only up to the last non-zero word and checks the 232-byte limit), at 0x1002AF00: below boot1,
 * above every application section. */
-e tramp_entry

MEMORY
{
    TRAMP : o = 0x1002AF00, l = 0x00000100
}

SECTIONS
{
    .tramp > TRAMP
    .cinit > TRAMP
}
