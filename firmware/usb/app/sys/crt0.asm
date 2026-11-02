;--------------------------------------------------------------------------------------------------------------
; Reset vector, interrupt vector and C start-up, replacing SDCC's (126 bytes there, 33 here): SDCC 4.5 emits its
; vector table and atomic support routines in the module defining main(), hence the entry point oc_main.
; Same start-up as SDCC crtstart/crtclear, plus PSW = 0 (the C code assumes register bank 0). The INT0, Timer 0 and
; INT1 vectors hold start-up code: those interrupts are never enabled. External RAM is not cleared.
; This module must be the first one of the link: it fixes the order of the areas.
;--------------------------------------------------------------------------------------------------------------
	.module crt0
	.globl	_timer1_isr
	.globl	_oc_main
	.globl	__start__stack

	.area	RSEG    (ABS,DATA)
	.area	REG_BANK_0	(REL,OVR,DATA)
	.ds	8
	.area	DSEG    (DATA)
	.area	SSEG
__start__stack:
	.ds	1
	.area	ISEG    (DATA)
	.area	IABS    (ABS,DATA)
	.area	BSEG    (BIT)
	.area	PSEG    (PAG,XDATA)
	.area	XSEG    (XDATA)
	.area	XABS    (ABS,XDATA)
	.area	HOME    (CODE)
	.area	GSINIT0 (CODE)
	.area	GSINIT1 (CODE)
	.area	GSINIT2 (CODE)
	.area	GSINIT3 (CODE)
	.area	GSINIT4 (CODE)
	.area	GSINIT5 (CODE)
	.area	GSINIT  (CODE)
	.area	GSFINAL (CODE)
	.area	CSEG    (CODE)
	.area	CONST   (CODE)

	.area	HOME    (CODE)
__interrupt_vect:
__sdcc_gsinit_startup::
	mov	sp, #__start__stack - 1		; 0x0000 reset (the ROM DFU starts the RAM code here)
	mov	psw, #0x00			; register bank 0
	clr	a
	mov	r0, #0xFF			; --iram-size 256 (Makefile)
00001$:	mov	@r0, a
	djnz	r0, 00001$
	sjmp	__gsinit			; C initialisers, right after this table
	.ds	0x001B - 0x000E			; 0x000E..0x001A: unused
	ljmp	_timer1_isr			; 0x001B Timer 1 (1 ms tick)
__gsinit:

	.area	GSFINAL (CODE)
	ljmp	_oc_main
