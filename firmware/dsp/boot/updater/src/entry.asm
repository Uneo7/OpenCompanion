; Updater entry point (RAM loader JUMP target): interrupts off, own stack, then updater_main (never returns).

        .global updater_entry
        .global updater_main
        .global __stack_top
        .global __TI_STATIC_BASE

        .sect   ".text:entry"
updater_entry:
        MVC     CSR, B0
        AND     -2, B0, B0              ; GIE = 0
        MVC     B0, CSR
        ZERO    B1
        MVC     B1, IER
        MVKL    __stack_top, B15
        MVKH    __stack_top, B15
        MVKL    __TI_STATIC_BASE, B14
        MVKH    __TI_STATIC_BASE, B14
        MVKL    updater_main, B0
        MVKH    updater_main, B0
        B       B0
        NOP     5
