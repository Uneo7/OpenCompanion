# The DA705 core is a C67x+: TI C6000 CGT 7.4.x only (-mv67p), as for the firmware and the RAM loader.
# The EEPROM image itself is assembled by the host tool: app/crates/oc-ais, binary `oc-bootimg`.

ROOT  := ../../../..
CGT74 ?= $(ROOT)/ti-cgt-c6000_7.4.24
ifeq ($(wildcard $(CGT74)/bin/cl6x*)$(filter clean,$(MAKECMDGOALS)),)
  $(error CGT 7.4.x not found in $(CGT74). Install TI C6000 CGT 7.4.24 there)
endif

CC     := $(CGT74)/bin/cl6x
OUT    := build
# -O3 -pm (whole program) --opt_for_space=3: boot1 must fit in 9,340 bytes above the stock image (LAYOUT.md).
CFLAGS := -mv67p --abi=eabi -O3 -pm --opt_for_space=3 --mem_model:data=near --mem_model:const=far \
          --display_error_number --emit_warnings_as_errors -I$(CGT74)/include -Iboot1/include
# 10063 = "entry point other than _c_int00": intended, there is no C runtime.
# boot_tag is never referenced by code: retained so that the host tool can check which build it is given.
LFLAGS := -z --zero_init=off --warn_sections --diag_suppress=10063 --emit_warnings_as_errors --retain=boot_tag

SHARED := boot1/src/ee.c boot1/src/crc32.c boot1/src/util.c boot1/src/start.c \
          boot1/src/rescue/link.c boot1/src/rescue/cmd.c boot1/src/rescue/dspint.c
BOOT1  := boot1/src/entry.asm boot1/src/main.c boot1/src/pll.c boot1/src/request.c boot1/src/stream.c \
          boot1/src/image.c boot1/src/lz4.c boot1/src/trial.c boot1/src/rescue/window.c $(SHARED)
UPDT   := updater/src/entry.asm updater/src/main.c updater/src/cmd_ee.c $(SHARED)
HDRS   := $(wildcard boot1/include/*.h)

all: $(OUT)/tramp.out $(OUT)/boot1.out $(OUT)/updater.out $(OUT)/updater_alt.out

$(OUT)/tramp $(OUT)/boot1 $(OUT)/updater $(OUT)/updater_alt:
	mkdir -p $@

$(OUT)/tramp.out: boot0/trampoline.asm boot0/lnk.cmd | $(OUT)/tramp
	$(CC) $(CFLAGS) --obj_directory=$(OUT)/tramp boot0/trampoline.asm $(LFLAGS) boot0/lnk.cmd -o $@ -m $(OUT)/tramp.map

$(OUT)/boot1.out: $(BOOT1) $(HDRS) boot1/lnk.cmd | $(OUT)/boot1
	$(CC) $(CFLAGS) --obj_directory=$(OUT)/boot1 $(BOOT1) $(LFLAGS) boot1/lnk.cmd -o $@ -m $(OUT)/boot1.map

$(OUT)/updater.out: $(UPDT) $(HDRS) updater/lnk.cmd | $(OUT)/updater
	$(CC) $(CFLAGS) -DBOOT_UPDATER --obj_directory=$(OUT)/updater $(UPDT) $(LFLAGS) updater/lnk.cmd -o $@ -m $(OUT)/updater.map

# Loadable while the default updater runs (tools/dsp_ee_recover.py --load-updater).
$(OUT)/updater_alt.out: $(UPDT) $(HDRS) updater/lnk_alt.cmd | $(OUT)/updater_alt
	$(CC) $(CFLAGS) -DBOOT_UPDATER --obj_directory=$(OUT)/updater_alt $(UPDT) $(LFLAGS) updater/lnk_alt.cmd -o $@ -m $(OUT)/updater_alt.map

clean:
	rm -rf $(OUT)

.PHONY: all clean
