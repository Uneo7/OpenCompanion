; boot1 entry point, linked at 0x1002B000 (first byte of the boot1 image, see lnk.cmd).
; Reached from the boot0 trampoline (ROM context: interrupts off, PLL bypassed), or through the RAM loader
; JUMP command for tests and migration. Interrupts off, own stack and data pointer, then boot1_main
; (never returns). No C runtime: boot1 initialises every variable it uses.

        .global boot1_entry
        .global boot1_main
        .global __stack_top
        .global __TI_STATIC_BASE

        .sect   ".text:entry"
boot1_entry:
        MVC     CSR, B0
        AND     -2, B0, B0              ; GIE = 0
        MVC     B0, CSR
        ZERO    B1
        MVC     B1, IER                 ; no interrupt source enabled
        MVKL    __stack_top, B15
        MVKH    __stack_top, B15
        MVKL    __TI_STATIC_BASE, B14
        MVKH    __TI_STATIC_BASE, B14
        MVKL    boot1_main, B0
        MVKH    boot1_main, B0
        B       B0                      ; boot1_main never returns
        NOP     5
