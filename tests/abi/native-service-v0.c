#include <stddef.h>
#include <stdint.h>

#include "specs/abi/native-network-bootstrap-v0.h"
#include "specs/abi/native-protocol-bootstrap-v0.h"
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
_Static_assert(offsetof(struct ginux_native_network_boot_v0, magic) == 0,
               "network bootstrap magic offset");
_Static_assert(offsetof(struct ginux_native_network_boot_v0, version) == 8,
               "network bootstrap version offset");
_Static_assert(offsetof(struct ginux_native_network_boot_v0, size) == 10,
               "network bootstrap size offset");
_Static_assert(offsetof(struct ginux_native_network_boot_v0, features) == 12,
               "network bootstrap features offset");
_Static_assert(
    offsetof(struct ginux_native_network_boot_v0, common_config_address) == 16,
    "network bootstrap common configuration offset");
_Static_assert(offsetof(struct ginux_native_network_boot_v0,
                        receive_notification_address) == 24,
               "network bootstrap receive notification offset");
_Static_assert(offsetof(struct ginux_native_network_boot_v0,
                        transmit_notification_address) == 32,
               "network bootstrap transmit notification offset");
_Static_assert(
    offsetof(struct ginux_native_network_boot_v0, dma_virtual_address) == 40,
    "network bootstrap DMA virtual offset");
_Static_assert(
    offsetof(struct ginux_native_network_boot_v0, dma_device_address) == 48,
    "network bootstrap DMA device offset");
_Static_assert(
    offsetof(struct ginux_native_network_boot_v0, device_capability) == 56,
    "network bootstrap capability offset");
_Static_assert(offsetof(struct ginux_native_network_boot_v0, mac_address) == 64,
               "network bootstrap MAC offset");
_Static_assert(offsetof(struct ginux_native_network_boot_v0, reserved) == 72,
               "network bootstrap reserved offset");
_Static_assert(GINUX_NATIVE_NETWORK_FEATURE_ARP_PROBE == UINT32_C(16),
               "network ARP feature bit");
_Static_assert(GINUX_NATIVE_NETWORK_FEATURE_RX_REUSE == UINT32_C(32),
               "network RX reuse feature bit");
_Static_assert(sizeof(struct ginux_native_network_boot_v1) == 88,
               "network bootstrap v1 size");
_Static_assert(offsetof(struct ginux_native_network_boot_v1,
                        packet_capability) == 72,
               "network packet capability offset");
_Static_assert(offsetof(struct ginux_native_network_boot_v1, reserved) == 80,
               "network bootstrap v1 reserved offset");
_Static_assert(sizeof(struct ginux_native_network_boot_v2) == 96,
               "network bootstrap v2 size");
_Static_assert(offsetof(struct ginux_native_network_boot_v2,
                        egress_capability) == 72,
               "network egress capability offset");
_Static_assert(offsetof(struct ginux_native_network_boot_v2,
                        expected_egress_sequence) == 80,
               "network expected egress sequence offset");
_Static_assert(offsetof(struct ginux_native_network_boot_v2, reserved) == 88,
               "network bootstrap v2 reserved offset");
_Static_assert(sizeof(struct ginux_native_network_boot_v3) == 96,
               "network bootstrap v3 size");
_Static_assert(offsetof(struct ginux_native_network_boot_v3,
                        egress_capability) == 72,
               "network v3 egress capability offset");
_Static_assert(offsetof(struct ginux_native_network_boot_v3,
                        expected_egress_sequence) == 80,
               "network v3 expected egress sequence offset");
_Static_assert(offsetof(struct ginux_native_network_boot_v3, reserved) == 88,
               "network bootstrap v3 reserved offset");
_Static_assert(sizeof(struct ginux_native_packet_request_v0) == 32,
               "native packet request size");
_Static_assert(offsetof(struct ginux_native_packet_request_v0,
                        expected_sequence) == 24,
               "native packet sequence offset");
_Static_assert(GINUX_NATIVE_NETWORK_FEATURE_PACKET_HANDOFF == UINT32_C(64),
               "network packet handoff feature bit");
_Static_assert(GINUX_NATIVE_NETWORK_FEATURE_PACKET_FAULT == UINT32_C(128),
               "network packet fault feature bit");
_Static_assert(GINUX_NATIVE_NETWORK_FEATURE_PACKET_BATCH == UINT32_C(256),
               "network packet batch feature bit");
_Static_assert(GINUX_NATIVE_NETWORK_FEATURE_PACKET_EGRESS == UINT32_C(512),
               "network packet egress feature bit");
_Static_assert(GINUX_NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS == UINT32_C(1024),
               "network reliable egress feature bit");
_Static_assert(GINUX_NATIVE_NETWORK_FEATURE_EGRESS_FAULT == UINT32_C(2048),
               "network egress fault feature bit");
_Static_assert(GINUX_NATIVE_NETWORK_FEATURE_EGRESS_WAIT == UINT32_C(4096),
               "network egress wait feature bit");
_Static_assert(GINUX_NATIVE_NETWORK_FEATURE_EGRESS_REUSE == UINT32_C(8192),
               "network egress reuse feature bit");
_Static_assert(GINUX_NATIVE_NETWORK_FEATURE_EGRESS_BATCH == UINT32_C(16384),
               "network egress batch feature bit");
_Static_assert(GINUX_NATIVE_NETWORK_FEATURE_EGRESS_RESIDENT == UINT32_C(32768),
               "network egress resident feature bit");
_Static_assert(GINUX_NATIVE_NETWORK_BOOT_V5_VERSION == UINT16_C(5),
               "network bootstrap v5 version");
_Static_assert(sizeof(struct ginux_native_network_boot_v5) == 96,
               "network bootstrap v5 size");
_Static_assert(GINUX_NATIVE_NETWORK_BOOT_V6_VERSION == UINT16_C(6),
               "network bootstrap v6 version");
_Static_assert(sizeof(struct ginux_native_network_boot_v6) == 96,
               "network bootstrap v6 size");
_Static_assert(GINUX_NATIVE_NETWORK_BOOT_V7_VERSION == UINT16_C(7),
               "network bootstrap v7 version");
_Static_assert(sizeof(struct ginux_native_network_boot_v7) == 96,
               "network bootstrap v7 size");
_Static_assert(GINUX_NATIVE_PACKET_EGRESS_CLAIM_OPERATION == UINT64_C(26),
               "native packet egress claim operation");
_Static_assert(GINUX_NATIVE_PACKET_EGRESS_ACK_OPERATION == UINT64_C(27),
               "native packet egress ack operation");
_Static_assert(GINUX_NATIVE_PACKET_EGRESS_DRAIN_WAIT_OPERATION == UINT64_C(30),
               "native packet egress drain wait operation");
_Static_assert(GINUX_NATIVE_PACKET_QUEUE_CAPACITY == UINT32_C(4),
               "native packet queue capacity");
_Static_assert(GINUX_NATIVE_CONTROL_FEATURE_PACKET_HANDOFF == UINT16_C(2048),
               "control packet handoff feature bit");
_Static_assert(GINUX_NATIVE_HOST_INSPECT_OPERATION == UINT64_C(32),
               "host inspection operation");
_Static_assert(GINUX_NATIVE_HOST_PROCESS_KIND_CONTAINER_INIT == UINT16_C(2),
               "container init host process kind");
_Static_assert(GINUX_NATIVE_CONTAINER_CONTROL_OPERATION == UINT64_C(33),
               "container control operation");
_Static_assert(GINUX_NATIVE_CONTAINER_HOST_SHUTDOWN == UINT64_C(4),
               "container host shutdown command");
_Static_assert(GINUX_NATIVE_CONTAINER_STATE_FAILED == UINT64_C(3),
               "failed container state");
_Static_assert(GINUX_NATIVE_PACKET_CONSUME_OPERATION == UINT64_C(22),
               "native packet consume operation");
_Static_assert(GINUX_NATIVE_PACKET_WAIT_OPERATION == UINT64_C(23),
               "native packet wait operation");
_Static_assert(GINUX_NATIVE_NETWORK_WAIT_TX_NOTIFIED == UINT64_C(1),
               "network TX wait command");
_Static_assert(GINUX_NATIVE_NETWORK_WAIT_RX_POSTED == UINT64_C(2),
               "network RX wait command");
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
_Static_assert(offsetof(struct ginux_native_protocol_boot_v0,
                        packet_capability) == 16,
               "protocol packet capability offset");
_Static_assert(offsetof(struct ginux_native_protocol_boot_v0,
                        expected_sequence) == 24,
               "protocol expected sequence offset");
_Static_assert(offsetof(struct ginux_native_protocol_boot_v0, local_mac) == 32,
               "protocol local MAC offset");
_Static_assert(offsetof(struct ginux_native_protocol_boot_v0, local_ipv4) == 40,
               "protocol local IPv4 offset");
_Static_assert(offsetof(struct ginux_native_protocol_boot_v0, peer_ipv4) == 44,
               "protocol peer IPv4 offset");
_Static_assert(offsetof(struct ginux_native_protocol_boot_v0, reserved) == 48,
               "protocol reserved offset");
_Static_assert(offsetof(struct ginux_native_protocol_boot_v1, packet_count) ==
                   48,
               "protocol packet count offset");
_Static_assert(offsetof(struct ginux_native_protocol_boot_v1, reserved) == 52,
               "protocol v1 reserved offset");
_Static_assert(offsetof(struct ginux_native_protocol_boot_v2, packet_count) ==
                   48,
               "protocol v2 packet count offset");
_Static_assert(offsetof(struct ginux_native_protocol_boot_v2, reserved) == 52,
               "protocol v2 reserved offset");
_Static_assert(offsetof(struct ginux_native_protocol_boot_v3, packet_count) ==
                   48,
               "protocol v3 packet count offset");
_Static_assert(offsetof(struct ginux_native_protocol_boot_v3, reserved) == 52,
               "protocol v3 reserved offset");
_Static_assert(offsetof(struct ginux_native_protocol_boot_v4, packet_count) ==
                   48,
               "protocol v4 packet count offset");
_Static_assert(offsetof(struct ginux_native_protocol_boot_v4, reserved) == 52,
               "protocol v4 reserved offset");

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

uint64_t ginux_native_network_v0_consumer(
    const struct ginux_native_network_boot_v0 *boot) {
    if (boot == NULL || boot->magic != GINUX_NATIVE_NETWORK_BOOT_MAGIC ||
        boot->version != GINUX_NATIVE_NETWORK_BOOT_VERSION ||
        boot->size != GINUX_NATIVE_NETWORK_BOOT_SIZE ||
        boot->features != GINUX_NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK ||
        boot->common_config_address == 0 ||
        boot->receive_notification_address == 0 ||
        boot->transmit_notification_address == 0 ||
        boot->receive_notification_address ==
            boot->transmit_notification_address ||
        boot->dma_virtual_address == 0 || boot->dma_device_address == 0 ||
        boot->device_capability == 0 || boot->mac_address == 0 ||
        (boot->mac_address >> 48) != 0 || boot->reserved != 0) {
        return UINT64_MAX;
    }
    return boot->device_capability;
}
