; boot0 trampoline (<= 232 bytes, so that boot0 = magic + Section Load + Jump_Close fits in 2 EEPROM pages).
;
; Loaded at 0x1002AF00 by the ROM bootloader (AIS Section Load) and entered through Jump_Close, in the ROM
; context: interrupts off, PLL bypassed, I2C1 configured by the ROM as master for U7001 (device 0x50).
; It reads EE_BOOT1_LEN (0x2480) bytes from EEPROM address 0xDB00 into 0x1002B000 and branches there.
;
; I2C1 handling:
; - Module reset (IRS = 0, then IRS = 1): clears any stale status left by the ROM. SPRU175D section 6: status
;   bits are forced to their defaults, the other registers (prescaler, clock dividers) keep the ROM values,
;   so the ROM bus speed is reused. The Bose stub does the same reset at the same point of the boot
;   (u7001_dsp.bin 0x0010, hardware.md section 1) and the ROM keeps reading the EEPROM afterwards.
; - Count mode: 2 address bytes without STOP, then repeated START, 0x2480 bytes received, automatic STOP
;   (same sequence as boot1's ee.c). The first address byte is written to ICDXR before STT, as in ee.c.
; - No timeout: if U7001 does not answer, this loops like the ROM does on a read failure. Recovery is the
;   same as for the ROM: DSPRESET (which also power-cycles U7001 through Q7100).
; - L1P is invalidated before the branch (as the Bose boot driver at 0x1000295C).

I2C1      .set  0x4A000000
BOOT1_EE  .set  0xDB                ; EEPROM address high byte (EE_BOOT1 = 0xDB00), low byte = 0
BOOT1_RAM .set  0x1002B000
BOOT1_LEN .set  0x2480
L1PICR    .set  0x20000004

        .global tramp_entry
        .sect   ".tramp"
tramp_entry:
        MVKL    I2C1, A4
        MVKH    I2C1, A4                ; A4 = I2C1 registers
        ZERO    B0
        STW     B0, *A4[9]              ; ICMDR = 0: module reset, status cleared
        MVK     0x20, B0
        STW     B0, *A4[9]              ; ICMDR = IRS: out of reset, ROM clock settings kept
        MVK     0x50, B0
        STW     B0, *A4[7]              ; ICSAR = 0x50 (U7001)
        MVK     2, B0
        STW     B0, *A4[5]              ; ICCNT = 2
        MVK     0x0620, B0              ; IRS | TRX | MST, no START yet
        STW     B0, *A4[9]
        MVK     BOOT1_EE, B0            ; first byte loaded into ICDXR BEFORE STT (never a stale ICDXR byte)
        STW     B0, *A4[8]              ; ICDXR
        MVK     0x2620, B0              ; IRS | TRX | MST | STT (no STP)
        STW     B0, *A4[9]
tx:     LDW     *A4[2], B0              ; ICSTR
        NOP     4
        EXTU    B0, 27, 31, B0          ; ICXRDY (bit 4): first byte moved to the shift register
  [!B0] B       tx
        NOP     5
        ZERO    B2
        STW     B2, *A4[8]              ; ICDXR = second byte (0x00)
ar:     LDW     *A4[2], B0
        NOP     4
        EXTU    B0, 29, 31, B0          ; ICARDY (bit 2): both address bytes sent
  [!B0] B       ar
        NOP     5
        MVK     BOOT1_LEN, A1           ; byte counter
        STW     A1, *A4[5]              ; ICCNT = boot1 length
        MVK     0x2C20, B0              ; IRS | MST | STP | STT: repeated START, receive, STOP at the end
        STW     B0, *A4[9]
        MVKL    BOOT1_RAM, A5
        MVKH    BOOT1_RAM, A5
        MV      A5, B3                  ; branch target, kept for the end
rx:     LDW     *A4[2], B0
        NOP     4
        EXTU    B0, 28, 31, B0          ; ICRRDY (bit 3)
  [!B0] B       rx
        NOP     5
        LDW     *A4[6], B2              ; ICDRR
        SUB     A1, 1, A1
        NOP     3
        STB     B2, *A5++
   [A1] B       rx
        NOP     5
        MVKL    L1PICR, A6
        MVKH    L1PICR, A6
        ZERO    B1
        MVKH    0x80000000, B1
        STW     B1, *A6                 ; invalidate the whole L1P
        MVK     1000, A2
dly:    SUB     A2, 1, A2
   [A2] B       dly
        NOP     5
        B       B3                      ; boot1_entry
        NOP     5
