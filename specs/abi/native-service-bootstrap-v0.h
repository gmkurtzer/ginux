#ifndef GINUX_NATIVE_SERVICE_BOOTSTRAP_V0_H
#define GINUX_NATIVE_SERVICE_BOOTSTRAP_V0_H

#include <stdint.h>

#define GINUX_NATIVE_SERVICE_BOOT_MAGIC UINT64_C(0x30425358554e4947)
#define GINUX_NATIVE_SERVICE_BOOT_VERSION UINT16_C(0)
#define GINUX_NATIVE_SERVICE_BOOT_SIZE UINT16_C(64)
#define GINUX_NATIVE_SERVICE_FEATURE_VIRTIO_BLOCK UINT32_C(1)

struct ginux_native_service_boot_v0 {
    uint64_t magic;
    uint16_t version;
    uint16_t size;
    uint32_t features;
    uint64_t common_config_address;
    uint64_t queue_notification_address;
    uint64_t dma_virtual_address;
    uint64_t dma_device_address;
    uint64_t device_capability;
    uint64_t reserved;
};

_Static_assert(sizeof(struct ginux_native_service_boot_v0) == 64,
               "ginux native service bootstrap v0 size");

#endif
