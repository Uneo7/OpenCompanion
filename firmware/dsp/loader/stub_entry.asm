; Called in place of a Bose graph module's process function (A4 = module, B4 = wires, A6 = 96, B3 = return):
; it never returns. Interrupts are disabled and we switch to our own stack inside the loader area, because the
; image loaded afterwards may overwrite the Bose task stacks.

        .global stub_entry
        .global loader_main
        .global __stack_top
        .global __TI_STATIC_BASE

        .sect   ".text:entry"
stub_entry:
        MVC     CSR, B0
        AND     -2, B0, B0              ; GIE = 0
        MVC     B0, CSR
        ZERO    B1
        MVC     B1, IER                 ; no interrupts
        MVKL    __stack_top, B15
        MVKH    __stack_top, B15
        MVKL    __TI_STATIC_BASE, B14
        MVKH    __TI_STATIC_BASE, B14
        MVKL    loader_main, B0
        MVKH    loader_main, B0
        B       B0                      ; loader_main never returns
        NOP     5
