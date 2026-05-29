KDIR ?= /lib/modules/$(shell uname -r)/build
RUST_MODULE_DIR := rust/corsair_wmi_kernel

# Top-level convenience wrapper. The nested Makefile is the Kbuild file for the
# external Rust module; this wrapper routes builds through the script so the
# distro rustc/RUST_LIB_SRC setup is consistent.
all:
	KDIR=$(KDIR) ./scripts/build.sh

clean:
	$(MAKE) -C $(RUST_MODULE_DIR) KDIR=$(KDIR) clean
	rm -f $(CURDIR)/corsair_wmi.ko
