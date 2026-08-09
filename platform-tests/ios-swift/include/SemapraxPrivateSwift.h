#ifndef SEMAPRAX_PRIVATE_SWIFT_H
#define SEMAPRAX_PRIVATE_SWIFT_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif
typedef struct spx_private_apple_swift_evidence_v1 {
  uint64_t words[8];
} spx_private_apple_swift_evidence_v1;

uint64_t spx_private_apple_swift_fixture_v1_open(void);
uint64_t spx_private_apple_swift_v1_adopt_pair(uint64_t first_payload,
                                               uint64_t second_payload,
                                               uint64_t *out_handle);
uint64_t spx_private_apple_swift_v1_consume(
    uint64_t handle, spx_private_apple_swift_evidence_v1 *out_evidence,
    uint32_t evidence_len);
uint64_t spx_private_apple_swift_v1_close_runtime(void);

#ifdef __cplusplus
}
#endif

#endif
