#ifndef GINUX_NATIVE_NETWORK_BOOTSTRAP_V0_H
#define GINUX_NATIVE_NETWORK_BOOTSTRAP_V0_H

#include <stdint.h>

#define GINUX_NATIVE_NETWORK_BOOT_MAGIC UINT64_C(0x30424e58554e4947)
#define GINUX_NATIVE_NETWORK_BOOT_VERSION UINT16_C(0)
#define GINUX_NATIVE_NETWORK_BOOT_SIZE UINT16_C(80)
#define GINUX_NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK UINT32_C(1)
#define GINUX_NATIVE_NETWORK_FEATURE_EMPTY_QUEUES UINT32_C(2)
#define GINUX_NATIVE_NETWORK_FEATURE_TRANSMIT_PROBE UINT32_C(4)
#define GINUX_NATIVE_NETWORK_FEATURE_TX_INTERRUPT UINT32_C(8)
#define GINUX_NATIVE_NETWORK_FEATURE_ARP_PROBE UINT32_C(16)
#define GINUX_NATIVE_NETWORK_FEATURE_RX_REUSE UINT32_C(32)
#define GINUX_NATIVE_NETWORK_FEATURE_PACKET_HANDOFF UINT32_C(64)
#define GINUX_NATIVE_NETWORK_FEATURE_PACKET_FAULT UINT32_C(128)
#define GINUX_NATIVE_NETWORK_FEATURE_PACKET_BATCH UINT32_C(256)
#define GINUX_NATIVE_NETWORK_BOOT_V1_VERSION UINT16_C(1)
#define GINUX_NATIVE_NETWORK_BOOT_V1_SIZE UINT16_C(88)
#define GINUX_NATIVE_PACKET_PUBLISH_OPERATION UINT64_C(21)
#define GINUX_NATIVE_PACKET_CONSUME_OPERATION UINT64_C(22)
#define GINUX_NATIVE_PACKET_WAIT_OPERATION UINT64_C(23)
#define GINUX_NATIVE_PACKET_WAIT_TIMEOUT_OPERATION UINT64_C(24)
#define GINUX_NATIVE_PACKET_SPACE_WAIT_OPERATION UINT64_C(25)
#define GINUX_NATIVE_PACKET_REQUEST_VERSION UINT16_C(0)
#define GINUX_NATIVE_PACKET_REQUEST_SIZE UINT32_C(32)
#define GINUX_NATIVE_PACKET_MAX_SIZE UINT32_C(1514)
#define GINUX_NATIVE_PACKET_QUEUE_CAPACITY UINT32_C(4)
#define GINUX_NATIVE_PACKET_WAIT_TIMED_OUT (UINT64_MAX - UINT64_C(5))
#define GINUX_NATIVE_PACKET_WAIT_MAX_TIMEOUT_MS UINT64_C(1000)

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

struct ginux_native_network_boot_v1 {
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
    uint64_t packet_capability;
    uint64_t reserved;
};

struct ginux_native_packet_request_v0 {
    uint32_t size;
    uint16_t version;
    uint16_t reserved;
    uint64_t buffer_address;
    uint32_t buffer_size;
    uint32_t reserved1;
    uint64_t expected_sequence;
};

_Static_assert(sizeof(struct ginux_native_network_boot_v1) == 88,
               "ginux native network bootstrap v1 size");
_Static_assert(sizeof(struct ginux_native_packet_request_v0) == 32,
               "ginux native packet request v0 size");

#endif
