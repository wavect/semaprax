#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdalign.h>
typedef struct { uint8_t *payload; uint64_t len; } State;
static uint64_t requested;
static uint64_t copies, drops, closes;
void spx_frozen_configure(uint64_t len) { requested=len; copies=drops=closes=0; }
uint64_t spx_frozen_count(uint32_t kind) { return kind==0?copies:kind==1?drops:closes; }
uint64_t spx_owned_data_context_size_v1(void) { return sizeof(State); }
uint64_t spx_owned_data_context_align_v1(void) { return alignof(State); }
uint32_t spx_owned_data_context_init_v1(State *state,uint64_t len) {
 if(!state || len!=sizeof(State)) return 3;
 state->payload=NULL; state->len=0; return 0;
}
uint32_t spx_owned_data_context_drop_v1(State *state) {
 if(!state || state->payload) return 3;
 closes++; return 0;
}
uint32_t spx_owned_data_call_spx_fixture_dot_value_v1(State *state,uint32_t *tag,uint64_t *handle,int64_t *error) {
 if(!state || state->payload || requested>65536 || !tag || !handle || !error) return 3;
 uint8_t *payload=(uint8_t*)malloc(requested?requested:1);
 if(!payload) return 4;
 memset(payload,0xff,requested);
 state->payload=payload; state->len=requested; *tag=0; *handle=1; *error=0; return 0;
}
uint32_t spx_owned_bytes_len_v1(State *state,uint64_t handle,uint64_t *len) {
 if(!state || !state->payload || handle!=1 || !len) return 3;
 *len=state->len; return 0;
}
uint32_t spx_owned_bytes_copy_v1(State *state,uint64_t handle,uint8_t *output,uint64_t len) {
 if(!state || !state->payload || handle!=1 || len!=state->len) return 3;
 if(len) { if(!output || output==state->payload) return 3; memcpy(output,state->payload,len); }
 else if(output) return 3;
 copies++; return 0;
}
uint32_t spx_owned_bytes_drop_v1(State *state,uint64_t handle) {
 if(!state || !state->payload || handle!=1) return 3;
 uint8_t *payload=state->payload; state->payload=NULL;
 memset(payload,0x55,state->len); free(payload); drops++; return 0;
}
