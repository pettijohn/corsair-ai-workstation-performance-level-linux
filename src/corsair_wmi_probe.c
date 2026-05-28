// SPDX-License-Identifier: BSD-2-Clause OR GPL-2.0-only
/*
 * Read-only WMI probe for the CORSAIR AI Workstation power selector.
 *
 * This driver binds to the WMI GUIDs seen on Linux and logs the raw notify
 * payload. By default it does not invoke methods; query_current=1 invokes only
 * WMAA method id 2, which the firmware AML shows as a read of EC0.FCMO.
 */

#include <linux/acpi.h>
#include <linux/device.h>
#include <linux/module.h>
#include <linux/printk.h>
#include <linux/wmi.h>

#define CORSAIR_EVENT_GUID  "8FAFC061-22DA-46E2-91DB-1FE3D7E5FF3C"
#define CORSAIR_METHOD_GUID "99D89064-8D50-42BB-BEA9-155B2E5D0FCD"

static bool query_blocks;
module_param(query_blocks, bool, 0444);
MODULE_PARM_DESC(query_blocks, "Query WMI data blocks at probe time; read-only, default false");

static bool query_current;
module_param(query_current, bool, 0444);
MODULE_PARM_DESC(query_current, "Invoke AA method id 2 at probe time to read current mode; default false");

static bool log_other_events;
module_param(log_other_events, bool, 0644);
MODULE_PARM_DESC(log_other_events, "Log non-selector WMI events too; default false");

static void log_acpi_object(const char *prefix, union acpi_object *obj, int depth);

static const char *event_mode_name(u8 detail)
{
	switch (detail) {
	case 0x11:
		return "Quiet";
	case 0x12:
		return "Balanced";
	case 0x13:
		return "Max";
	case 0x14:
		return "Super";
	default:
		return "unknown";
	}
}

static bool is_selector_event(const u8 *data, size_t length)
{
	if (!data || length < 3)
		return false;

	return data[0] == 0x01 && data[2] == 0x81 &&
	       data[1] >= 0x11 && data[1] <= 0x14;
}

static const char *method_mode_name(u64 value)
{
	switch (value) {
	case 0:
		return "Balanced";
	case 1:
		return "Max";
	case 2:
		return "Quiet";
	case 3:
		return "Super";
	default:
		return "unknown";
	}
}

static void log_event_detail(const char *prefix, const u8 *data, size_t length)
{
	if (!data || length < 3)
		return;

	pr_info("%s EventDetail=[0x%02x,0x%02x,0x%02x] mode=%s\n",
		prefix, data[0], data[1], data[2], event_mode_name(data[1]));
}

static const char *acpi_type_name(u32 type)
{
	switch (type) {
	case ACPI_TYPE_INTEGER:
		return "integer";
	case ACPI_TYPE_STRING:
		return "string";
	case ACPI_TYPE_BUFFER:
		return "buffer";
	case ACPI_TYPE_PACKAGE:
		return "package";
	case ACPI_TYPE_LOCAL_REFERENCE:
		return "reference";
	default:
		return "other";
	}
}

static void log_package(const char *prefix, union acpi_object *obj, int depth)
{
	u32 i;

	pr_info("%s package count=%u\n", prefix, obj->package.count);
	if (depth >= 3)
		return;

	for (i = 0; i < obj->package.count; i++) {
		char child_prefix[64];

		snprintf(child_prefix, sizeof(child_prefix), "%s[%u]", prefix, i);
		log_acpi_object(child_prefix, &obj->package.elements[i], depth + 1);
	}
}

static void log_acpi_object(const char *prefix, union acpi_object *obj, int depth)
{
	if (!obj) {
		pr_info("%s null object\n", prefix);
		return;
	}

	pr_info("%s type=%s(%u)\n", prefix, acpi_type_name(obj->type), obj->type);

	switch (obj->type) {
	case ACPI_TYPE_INTEGER:
		pr_info("%s integer=0x%llx (%llu)\n",
			prefix, obj->integer.value, obj->integer.value);
		pr_info("%s decoded current mode=%s\n",
			prefix, method_mode_name(obj->integer.value));
		break;
	case ACPI_TYPE_STRING:
		pr_info("%s string len=%u value=\"%.*s\"\n",
			prefix, obj->string.length, obj->string.length,
			obj->string.pointer);
		break;
	case ACPI_TYPE_BUFFER:
		pr_info("%s buffer len=%u\n", prefix, obj->buffer.length);
		log_event_detail(prefix, obj->buffer.pointer, obj->buffer.length);
		print_hex_dump(KERN_INFO, "corsair_wmi_probe: buffer ",
			       DUMP_PREFIX_OFFSET, 16, 1, obj->buffer.pointer,
			       obj->buffer.length, false);
		break;
	case ACPI_TYPE_PACKAGE:
		log_package(prefix, obj, depth);
		break;
	default:
		break;
	}
}

static void corsair_notify_new(struct wmi_device *wdev, const struct wmi_buffer *data)
{
	if (!data || !data->data || !data->length)
		return;

	if (!is_selector_event(data->data, data->length) && !log_other_events)
		return;

	dev_info(&wdev->dev, "notify length=%zu\n", data->length);
	print_hex_dump(KERN_INFO, "corsair_wmi_probe: notify ",
		       DUMP_PREFIX_OFFSET, 16, 1, data->data, data->length,
		       false);
	log_event_detail("corsair_wmi_probe: notify", data->data, data->length);
}

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
		char prefix[64];

		obj = wmidev_block_query(wdev, i);
		snprintf(prefix, sizeof(prefix), "corsair_wmi_probe: query[%u]", i);
		if (!obj || IS_ERR(obj)) {
			dev_info(&wdev->dev, "query instance %u failed: %ld\n",
				 i, obj ? PTR_ERR(obj) : -ENODATA);
			continue;
		}
		log_acpi_object(prefix, obj, 0);
		kfree(obj);
	}
}

static void try_query_current(struct wmi_device *wdev)
{
	struct acpi_buffer in = { 0, NULL };
	struct acpi_buffer out = { ACPI_ALLOCATE_BUFFER, NULL };
	acpi_status status;

	if (!query_current)
		return;

	if (strncasecmp(dev_name(&wdev->dev), CORSAIR_METHOD_GUID,
			strlen(CORSAIR_METHOD_GUID)) != 0)
		return;

	dev_info(&wdev->dev, "query_current enabled, invoking AA method id 2\n");
	status = wmidev_evaluate_method(wdev, 0, 2, &in, &out);
	if (ACPI_FAILURE(status)) {
		dev_info(&wdev->dev, "AA method id 2 failed: %s\n",
			 acpi_format_exception(status));
		return;
	}

	log_acpi_object("corsair_wmi_probe: current", out.pointer, 0);
	ACPI_FREE(out.pointer);
}

static int corsair_probe(struct wmi_device *wdev, const void *context)
{
	dev_info(&wdev->dev, "bound read-only probe dev_name=%s\n",
		 dev_name(&wdev->dev));
	try_query_block(wdev);
	try_query_current(wdev);
	return 0;
}

static void corsair_remove(struct wmi_device *wdev)
{
	dev_info(&wdev->dev, "removed read-only probe\n");
}

static const struct wmi_device_id corsair_wmi_id_table[] = {
	{ .guid_string = CORSAIR_EVENT_GUID },
	{ .guid_string = CORSAIR_METHOD_GUID },
	{ }
};
MODULE_DEVICE_TABLE(wmi, corsair_wmi_id_table);

static struct wmi_driver corsair_wmi_driver = {
	.driver = {
		.name = "corsair-wmi-probe",
	},
	.id_table = corsair_wmi_id_table,
	.no_singleton = true,
	.probe = corsair_probe,
	.remove = corsair_remove,
	.notify_new = corsair_notify_new,
};

module_wmi_driver(corsair_wmi_driver);

MODULE_AUTHOR("Local driver prototype");
MODULE_DESCRIPTION("Read-only CORSAIR AI Workstation performance mode WMI logger");
MODULE_LICENSE("Dual BSD/GPL");
