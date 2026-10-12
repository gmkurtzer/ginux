#include <stddef.h>
#include <stdint.h>

#include "specs/abi/native-service-bootstrap-v0.h"
#include "specs/abi/native-service-lifecycle-v0.h"

_Static_assert(offsetof(struct ginux_native_service_boot_v0, magic) == 0,
               "bootstrap magic offset");
_Static_assert(offsetof(struct ginux_native_service_boot_v0, version) == 8,
               "bootstrap version offset");
_Static_assert(offsetof(struct ginux_native_service_boot_v0, size) == 10,
               "bootstrap size offset");
_Static_assert(offsetof(struct ginux_native_service_boot_v0, features) == 12,
               "bootstrap features offset");
_Static_assert(
    offsetof(struct ginux_native_service_boot_v0, common_config_address) == 16,
    "bootstrap common configuration offset");
_Static_assert(
    offsetof(struct ginux_native_service_boot_v0, queue_notification_address) ==
        24,
    "bootstrap notification offset");
_Static_assert(
    offsetof(struct ginux_native_service_boot_v0, dma_virtual_address) == 32,
    "bootstrap DMA virtual offset");
_Static_assert(
    offsetof(struct ginux_native_service_boot_v0, dma_device_address) == 40,
    "bootstrap DMA device offset");
_Static_assert(
    offsetof(struct ginux_native_service_boot_v0, device_capability) == 48,
    "bootstrap capability offset");
_Static_assert(offsetof(struct ginux_native_service_boot_v0, reserved) == 56,
               "bootstrap reserved offset");
_Static_assert(offsetof(struct ginux_native_control_descriptor_v4, size) == 0,
               "control descriptor size offset");
_Static_assert(offsetof(struct ginux_native_control_descriptor_v4, version) ==
                   4,
               "control descriptor version offset");
_Static_assert(offsetof(struct ginux_native_control_descriptor_v4, features) ==
                   6,
               "control descriptor features offset");
_Static_assert(offsetof(struct ginux_native_control_descriptor_v4,
                        max_buffered_console_bytes) == 8,
               "control descriptor console limit offset");
_Static_assert(
    offsetof(struct ginux_native_control_descriptor_v4, reserved) == 12,
    "control descriptor reserved offset");

#define COMPATIBLE_DISCOVERY                                                   \
    GINUX_NATIVE_CONTROL_DISCOVERY(                                            \
        4, GINUX_NATIVE_CONTROL_REQUIRED_FEATURES | UINT16_C(2))

_Static_assert(GINUX_NATIVE_CONTROL_ABI_IS_COMPATIBLE(COMPATIBLE_DISCOVERY),
               "required features and unknown low feature are compatible");
_Static_assert(
    !GINUX_NATIVE_CONTROL_ABI_IS_COMPATIBLE(
        GINUX_NATIVE_CONTROL_DISCOVERY(3,
                                       GINUX_NATIVE_CONTROL_REQUIRED_FEATURES)),
    "older control ABI is incompatible");
_Static_assert(
    !GINUX_NATIVE_CONTROL_ABI_IS_COMPATIBLE(
        GINUX_NATIVE_CONTROL_DISCOVERY(
            4, GINUX_NATIVE_CONTROL_FEATURE_OPAQUE_CAPABILITIES)),
    "missing required feature is incompatible");
_Static_assert(!GINUX_NATIVE_CONTROL_ABI_IS_COMPATIBLE(
                   COMPATIBLE_DISCOVERY | (UINT64_C(1) << 32)),
               "reserved high bits are incompatible");

uint64_t ginux_native_service_v0_consumer(
    const struct ginux_native_service_boot_v0 *boot, uint64_t discovery,
    struct ginux_native_control_descriptor_v4 descriptor) {
    if (boot == NULL || boot->magic != GINUX_NATIVE_SERVICE_BOOT_MAGIC ||
        boot->version != GINUX_NATIVE_SERVICE_BOOT_VERSION ||
        boot->size != GINUX_NATIVE_SERVICE_BOOT_SIZE ||
        boot->features != GINUX_NATIVE_SERVICE_FEATURE_VIRTIO_BLOCK ||
        boot->reserved != 0 ||
        !GINUX_NATIVE_CONTROL_ABI_IS_COMPATIBLE(discovery) ||
        !GINUX_NATIVE_CONTROL_DESCRIPTOR_IS_COMPATIBLE(descriptor)) {
        return UINT64_MAX;
    }
    return boot->device_capability;
}
