#ifndef GINUX_NATIVE_PROTOCOL_BOOTSTRAP_V0_H
#define GINUX_NATIVE_PROTOCOL_BOOTSTRAP_V0_H

#include <stdint.h>

#define GINUX_NATIVE_PROTOCOL_BOOT_MAGIC UINT64_C(0x30425058554e4947)
#define GINUX_NATIVE_PROTOCOL_BOOT_VERSION UINT16_C(0)
#define GINUX_NATIVE_PROTOCOL_BOOT_SIZE UINT16_C(56)
#define GINUX_NATIVE_PROTOCOL_FEATURE_PACKET_CONSUMER UINT32_C(1)
#define GINUX_NATIVE_PROTOCOL_FEATURE_ETHERNET_ARP UINT32_C(2)
#define GINUX_NATIVE_PROTOCOL_FEATURE_WAIT_LOOP UINT32_C(4)
#define GINUX_NATIVE_PROTOCOL_BOOT_V1_VERSION UINT16_C(1)
#define GINUX_NATIVE_PROTOCOL_BOOT_V1_SIZE UINT16_C(56)

struct ginux_native_protocol_boot_v0 {
    uint64_t magic;
    uint16_t version;
    uint16_t size;
    uint32_t features;
    uint64_t packet_capability;
    uint64_t expected_sequence;
    uint64_t local_mac;
    uint32_t local_ipv4;
    uint32_t peer_ipv4;
    uint64_t reserved;
};

_Static_assert(sizeof(struct ginux_native_protocol_boot_v0) == 56,
               "native protocol bootstrap v0 size");

struct ginux_native_protocol_boot_v1 {
    uint64_t magic;
    uint16_t version;
    uint16_t size;
    uint32_t features;
    uint64_t packet_capability;
    uint64_t expected_sequence;
    uint64_t local_mac;
    uint32_t local_ipv4;
    uint32_t peer_ipv4;
    uint32_t packet_count;
    uint32_t reserved;
};

_Static_assert(sizeof(struct ginux_native_protocol_boot_v1) == 56,
               "native protocol bootstrap v1 size");

#endif
