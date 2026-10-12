#ifndef GINUX_NATIVE_NETWORK_BOOTSTRAP_V0_H
#define GINUX_NATIVE_NETWORK_BOOTSTRAP_V0_H

#include <stdint.h>

#define GINUX_NATIVE_NETWORK_BOOT_MAGIC UINT64_C(0x30424e58554e4947)
#define GINUX_NATIVE_NETWORK_BOOT_VERSION UINT16_C(0)
#define GINUX_NATIVE_NETWORK_BOOT_SIZE UINT16_C(80)
#define GINUX_NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK UINT32_C(1)

struct ginux_native_network_boot_v0 {
    uint64_t magic;
    uint16_t version;
    uint16_t size;
    uint32_t features;
    uint64_t common_config_address;
    uint64_t receive_notification_address;
    uint64_t transmit_notification_address;
    uint64_t dma_virtual_address;
    uint64_t dma_device_address;
    uint64_t device_capability;
    uint64_t mac_address;
    uint64_t reserved;
};

_Static_assert(sizeof(struct ginux_native_network_boot_v0) == 80,
               "ginux native network bootstrap v0 size");

#endif
