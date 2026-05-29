KDIR ?= /lib/modules/$(shell uname -r)/build

obj-m += corsair_wmi_rust.o

all:
	$(MAKE) -C $(KDIR) M=$(CURDIR) modules

clean:
	$(MAKE) -C $(KDIR) M=$(CURDIR) clean
