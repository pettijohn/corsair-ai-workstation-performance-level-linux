KDIR ?= /lib/modules/$(shell uname -r)/build

# Kbuild entry point for the out-of-tree Rust module. The repository root
# Makefile is only a convenience wrapper around this file.
obj-m += corsair_wmi.o

all: wmi_abi_check.c
	$(MAKE) -C $(KDIR) M=$(CURDIR) wmi_abi_check.o
	$(MAKE) -C $(KDIR) M=$(CURDIR) modules

wmi_abi_check.c: scripts/gen_wmi_abi_check.sh
	sh scripts/gen_wmi_abi_check.sh $@

clean:
	$(MAKE) -C $(KDIR) M=$(CURDIR) clean
	rm -f wmi_abi_check.c
