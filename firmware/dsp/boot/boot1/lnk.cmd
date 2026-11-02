/* boot1: code and constants in 0x1002B000..0x1002D47B, loaded by the trampoline from EEPROM 0xDB00 (0x2480
 * bytes; the last 4 bytes, RAM 0x1002D47C / EEPROM 0xFF7C, hold the CRC-32 added by the host tool).
 * 0x1002DFF0..0x1002DFF7: software rescue flag. Variables and stack in 0x1002E000..0x1002FFFF (not loaded,
 * initialised by the code: there is no C runtime). */
-e boot1_entry

MEMORY
{
    BOOT1 : o = 0x1002B000, l = 0x0000247C
    DATA  : o = 0x1002E000, l = 0x00001800
    STACK : o = 0x1002F800, l = 0x00000800
}

SECTIONS
{
    .text:entry : load = 0x1002B000
    .text       > BOOT1
    .const      > BOOT1
    .switch     > BOOT1
    .fardata    > BOOT1
    .neardata   > BOOT1
    .rodata     > BOOT1
    .cinit      > BOOT1
    .far        > DATA
    .bss        > DATA
}

__stack_top = 0x1002FFF8;
