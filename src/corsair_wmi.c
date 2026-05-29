// SPDX-License-Identifier: GPL-2.0
/*
 * Read-only WMI shim for the CORSAIR AI Workstation performance selector.
 *
 * The durable decode contract lives in the Rust core crate. This C module owns
 * the Linux WMI/sysfs boundary while Rust-for-Linux WMI support matures.
 */

#include <linux/acpi.h>
#include <linux/device.h>
#include <linux/module.h>
#include <linux/mutex.h>
#include <linux/printk.h>
#include <linux/string.h>
#include <linux/sysfs.h>
#include <linux/wmi.h>

#define CORSAIR_EVENT_GUID  "8FAFC061-22DA-46E2-91DB-1FE3D7E5FF3C"
#define CORSAIR_METHOD_GUID "99D89064-8D50-42BB-BEA9-155B2E5D0FCD"

#define CORSAIR_MODE_BALANCED 0
#define CORSAIR_MODE_MAX      1
#define CORSAIR_MODE_QUIET    2
#define CORSAIR_MODE_SUPER    3
#define CORSAIR_MODE_UNKNOWN  255

/*
 * Two WMI devices participate: one receives events and the other owns the AA
 * method used for the current-mode query. Sysfs files live on the method
 * device, so event callbacks keep a pointer to it for sysfs_notify().
 */
struct corsair_state {
	struct mutex lock;
	struct wmi_device *method_wdev;
	u8 mode;
};

static struct corsair_state corsair_state = {
	.lock = __MUTEX_INITIALIZER(corsair_state.lock),
	.mode = CORSAIR_MODE_UNKNOWN,
};

static bool log_other_events;
module_param(log_other_events, bool, 0644);
MODULE_PARM_DESC(log_other_events, "Log non-selector WMI events too; default false");

static bool query_blocks;
module_param(query_blocks, bool, 0444);
MODULE_PARM_DESC(query_blocks, "Query WMI data blocks at probe time; debug only, default false");

/* The WMI core binds this driver to both GUIDs; only the AA method device gets sysfs. */
static bool is_method_device(struct wmi_device *wdev)
{
	return strncasecmp(dev_name(&wdev->dev), CORSAIR_METHOD_GUID,
			   strlen(CORSAIR_METHOD_GUID)) == 0;
}

/* Sysfs exposes stable lowercase names, while the raw file exposes these numeric values. */
static const char *mode_name(u8 mode)
{
	switch (mode) {
	case CORSAIR_MODE_QUIET:
		return "quiet";
	case CORSAIR_MODE_BALANCED:
		return "balanced";
	case CORSAIR_MODE_MAX:
		return "max";
	case CORSAIR_MODE_SUPER:
		return "super";
	default:
		return "unknown";
	}
}

static u8 mode_from_query_value(u64 value)
{
	switch (value) {
	case 0:
		return CORSAIR_MODE_BALANCED;
	case 1:
		return CORSAIR_MODE_MAX;
	case 2:
		return CORSAIR_MODE_QUIET;
	case 3:
		return CORSAIR_MODE_SUPER;
	default:
		return CORSAIR_MODE_UNKNOWN;
	}
}

/* Event detail bytes use a different encoding than the read-current method. */
static u8 mode_from_event_detail(u8 detail)
{
	switch (detail) {
	case 0x11:
		return CORSAIR_MODE_QUIET;
	case 0x12:
		return CORSAIR_MODE_BALANCED;
	case 0x13:
		return CORSAIR_MODE_MAX;
	case 0x14:
		return CORSAIR_MODE_SUPER;
	default:
		return CORSAIR_MODE_UNKNOWN;
	}
}

static bool is_selector_event(const u8 *data, size_t length)
{
	if (!data || length < 3)
		return false;

	return data[0] == 0x01 && data[2] == 0x81 &&
	       data[1] >= 0x11 && data[1] <= 0x14;
}

/*
 * The cached mode is the single userspace-visible state. It is initialized from
 * the read-only method query and then updated by selector events.
 */
static void set_cached_mode(u8 mode, const char *source)
{
	struct wmi_device *method_wdev;
	bool changed;

	mutex_lock(&corsair_state.lock);
	changed = corsair_state.mode != mode;
	corsair_state.mode = mode;
	method_wdev = corsair_state.method_wdev;
	mutex_unlock(&corsair_state.lock);

	pr_info("corsair_wmi: mode=%s raw=%u source=%s%s\n",
		mode_name(mode), mode, source, changed ? "" : " unchanged");

	if (changed && method_wdev) {
		sysfs_notify(&method_wdev->dev.kobj, NULL, "current_mode");
		sysfs_notify(&method_wdev->dev.kobj, NULL, "current_mode_raw");
	}
}

/* Read-only userspace ABI: mode name for humans, raw value for scripts. */
static ssize_t current_mode_show(struct device *dev,
				 struct device_attribute *attr, char *buf)
{
	u8 mode;

	mutex_lock(&corsair_state.lock);
	mode = corsair_state.mode;
	mutex_unlock(&corsair_state.lock);

	return sysfs_emit(buf, "%s\n", mode_name(mode));
}
static DEVICE_ATTR_RO(current_mode);

static ssize_t current_mode_raw_show(struct device *dev,
				     struct device_attribute *attr, char *buf)
{
	u8 mode;

	mutex_lock(&corsair_state.lock);
	mode = corsair_state.mode;
	mutex_unlock(&corsair_state.lock);

	return sysfs_emit(buf, "%u\n", mode);
}
static DEVICE_ATTR_RO(current_mode_raw);

static int create_mode_attrs(struct wmi_device *wdev)
{
	int ret;

	ret = device_create_file(&wdev->dev, &dev_attr_current_mode);
	if (ret)
		return ret;

	ret = device_create_file(&wdev->dev, &dev_attr_current_mode_raw);
	if (ret) {
		device_remove_file(&wdev->dev, &dev_attr_current_mode);
		return ret;
	}

	return 0;
}

static void remove_mode_attrs(struct wmi_device *wdev)
{
	device_remove_file(&wdev->dev, &dev_attr_current_mode_raw);
	device_remove_file(&wdev->dev, &dev_attr_current_mode);
}

/*
 * AA method id 2 is the read-only current-mode path. Do not use method id 1
 * here; that method is reserved for firmware state changes.
 */
static int query_current_mode(struct wmi_device *wdev)
{
	struct acpi_buffer in = { 0, NULL };
	struct acpi_buffer out = { ACPI_ALLOCATE_BUFFER, NULL };
	union acpi_object *obj;
	acpi_status status;
	u8 mode;

	status = wmidev_evaluate_method(wdev, 0, 2, &in, &out);
	if (ACPI_FAILURE(status)) {
		dev_warn(&wdev->dev, "AA method id 2 failed: %s\n",
			 acpi_format_exception(status));
		return -EIO;
	}

	obj = out.pointer;
	if (!obj || obj->type != ACPI_TYPE_INTEGER) {
		dev_warn(&wdev->dev, "AA method id 2 returned non-integer\n");
		ACPI_FREE(out.pointer);
		return -ENODATA;
	}

	mode = mode_from_query_value(obj->integer.value);
	ACPI_FREE(out.pointer);

	set_cached_mode(mode, "query");
	return 0;
}

/* Optional diagnostic path retained for platform bring-up, not needed normally. */
static void try_query_block(struct wmi_device *wdev)
{
	union acpi_object *obj;
	u8 count;
	u8 i;

	if (!query_blocks)
		return;

	count = wmidev_instance_count(wdev);
	dev_info(&wdev->dev, "query_blocks enabled, instance_count=%u\n", count);

	for (i = 0; i < count; i++) {
		obj = wmidev_block_query(wdev, i);
		if (!obj || IS_ERR(obj)) {
			dev_info(&wdev->dev, "query instance %u failed: %ld\n",
				 i, obj ? PTR_ERR(obj) : -ENODATA);
			continue;
		}

		dev_info(&wdev->dev, "query instance %u returned ACPI type %u\n",
			 i, obj->type);
		kfree(obj);
	}
}

/*
 * The event GUID carries multiple firmware OSD events. Selector events are the
 * narrow 01:{11..14}:81 family; everything else is ignored unless debugging.
 */
static void corsair_notify_new(struct wmi_device *wdev, const struct wmi_buffer *data)
{
	u8 *payload;
	u8 mode;

	if (!data || !data->data || !data->length)
		return;

	payload = data->data;
	if (!is_selector_event(payload, data->length)) {
		if (log_other_events)
			dev_info(&wdev->dev,
				 "ignored non-selector event [%02x %02x %02x] len=%zu\n",
				 data->length > 0 ? payload[0] : 0,
				 data->length > 1 ? payload[1] : 0,
				 data->length > 2 ? payload[2] : 0,
				 data->length);
		return;
	}

	mode = mode_from_event_detail(payload[1]);
	dev_info(&wdev->dev, "selector event detail=0x%02x mode=%s\n",
		 payload[1], mode_name(mode));
	set_cached_mode(mode, "event");
}

/*
 * The method device owns sysfs and performs the initial query. The event device
 * only needs to bind so notify_new can receive selector changes.
 */
static int corsair_probe(struct wmi_device *wdev, const void *context)
{
	int ret;

	dev_info(&wdev->dev, "bound dev_name=%s\n", dev_name(&wdev->dev));
	try_query_block(wdev);

	if (!is_method_device(wdev))
		return 0;

	mutex_lock(&corsair_state.lock);
	corsair_state.method_wdev = wdev;
	mutex_unlock(&corsair_state.lock);

	ret = create_mode_attrs(wdev);
	if (ret) {
		mutex_lock(&corsair_state.lock);
		if (corsair_state.method_wdev == wdev)
			corsair_state.method_wdev = NULL;
		mutex_unlock(&corsair_state.lock);
		return ret;
	}

	query_current_mode(wdev);
	return 0;
}

/* Remove sysfs before dropping the method-device pointer used by notifications. */
static void corsair_remove(struct wmi_device *wdev)
{
	if (is_method_device(wdev)) {
		remove_mode_attrs(wdev);
		mutex_lock(&corsair_state.lock);
		if (corsair_state.method_wdev == wdev)
			corsair_state.method_wdev = NULL;
		mutex_unlock(&corsair_state.lock);
	}

	dev_info(&wdev->dev, "removed\n");
}

static const struct wmi_device_id corsair_wmi_id_table[] = {
	{ .guid_string = CORSAIR_EVENT_GUID },
	{ .guid_string = CORSAIR_METHOD_GUID },
	{ }
};
MODULE_DEVICE_TABLE(wmi, corsair_wmi_id_table);

static struct wmi_driver corsair_wmi_driver = {
	.driver = {
		.name = "corsair_wmi",
	},
	.id_table = corsair_wmi_id_table,
	.no_singleton = true,
	.probe = corsair_probe,
	.remove = corsair_remove,
	.notify_new = corsair_notify_new,
};

module_wmi_driver(corsair_wmi_driver);

MODULE_AUTHOR("Local driver prototype");
MODULE_DESCRIPTION("Read-only CORSAIR AI Workstation performance mode WMI driver");
MODULE_LICENSE("GPL");
