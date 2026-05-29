KDIR ?= /lib/modules/$(shell uname -r)/build

obj-m += corsair_wmi_rust_probe.o

all:
	$(MAKE) -C $(KDIR) M=$(CURDIR) modules

clean:
	$(MAKE) -C $(KDIR) M=$(CURDIR) clean
