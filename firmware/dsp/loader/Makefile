# The DA705 core is a C67x+: only TI C6000 CGT 7.4.x (and earlier) can target it (-mv67p).
# CGT 8.3 only builds for C674x (-mv6740), which may emit C64x+-only instructions: use it for
# compile checks only (make CHECK_ONLY=1), never load that output.
ROOT  := ../../../..
CGT74 ?= $(ROOT)/ti-cgt-c6000_7.4.24
CGT83 ?= $(ROOT)/ti-cgt-c6000_8.3.7

ifdef CHECK_ONLY
  CGT     := $(CGT83)
  SILICON := -mv6740 --no_compress
else
  CGT     := $(CGT74)
  SILICON := -mv67p
  ifeq ($(wildcard $(CGT74)/bin/cl6x*)$(filter clean,$(MAKECMDGOALS)),)
    $(error CGT 7.4.x not found in $(CGT74). Install TI C6000 CGT 7.4.24 there, or run "make CHECK_ONLY=1")
  endif
endif

CC     = $(CGT)/bin/cl6x
CFLAGS = $(SILICON) --abi=eabi -O2 --mem_model:data=far --mem_model:const=far --display_error_number -I$(CGT)/include
OUT    = build

all: $(OUT)/loader.out

$(OUT):
	mkdir -p $(OUT)

$(OUT)/loader.out: stub_entry.asm loader.c lnk.cmd | $(OUT)
	$(CC) $(CFLAGS) --obj_directory=$(OUT) stub_entry.asm loader.c -z --zero_init=off lnk.cmd -o $@ -m $(OUT)/loader.map

clean:
	rm -rf $(OUT)

.PHONY: all clean
