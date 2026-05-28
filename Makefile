KDIR ?= /lib/modules/$(shell uname -r)/build

obj-m += corsair_wmi.o
corsair_wmi-y := src/corsair_wmi.o

all:
	$(MAKE) -C $(KDIR) M=$(CURDIR) modules

clean:
	$(MAKE) -C $(KDIR) M=$(CURDIR) clean
