KDIR ?= /lib/modules/$(shell uname -r)/build

# Kbuild entry point for the out-of-tree Rust module. The repository root
# Makefile is only a convenience wrapper around this file.
obj-m += corsair_wmi.o

all:
	$(MAKE) -C $(KDIR) M=$(CURDIR) modules

clean:
	$(MAKE) -C $(KDIR) M=$(CURDIR) clean
