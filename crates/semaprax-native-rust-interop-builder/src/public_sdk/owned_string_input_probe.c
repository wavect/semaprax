#include "owner.h"
#include <stddef.h>
#include <string.h>
#define EXPECT(value) do { if(!(value)) return __LINE__; } while(0)
static uintptr_t source_address;
uintptr_t spx_input_source(void) { return source_address; }
int32_t spx_input_probe(uint64_t context) {
 spx_owner sentinel={UINT64_MAX,UINT64_MAX,UINT64_MAX}, output=sentinel;
 uint64_t sizes[]={4097,UINT64_MAX,UINT64_C(0x8000000000000000),UINT64_C(0x7fffffffffffffff)};
 for(size_t i=0;i<sizeof(sizes)/sizeof(sizes[0]);i++) {
  EXPECT(spx_owner_string_from_utf8(context,NULL,sizes[i],&output)==4);
  EXPECT(memcmp(&output,&sentinel,sizeof(output))==0);
 }
 int64_t signed_sizes[]={-1,INT64_MIN,INT64_MAX,4097};
 for(size_t i=0;i<sizeof(signed_sizes)/sizeof(signed_sizes[0]);i++) {
  EXPECT(spx_owner_string_from_utf8_signed(context,NULL,signed_sizes[i],&output)==4);
  EXPECT(memcmp(&output,&sentinel,sizeof(output))==0);
 }
 EXPECT(spx_owner_string_from_utf8(context,NULL,1,&output)==3);
 uint8_t invalid[][4]={{0xc0,0xaf,0,0},{0xed,0xa0,0x80,0},{0xf4,0x90,0x80,0x80},{0xf0,0x9f,0,0},{0x80,0,0,0}};
 uint64_t invalid_len[]={2,3,4,2,1};
 for(size_t i=0;i<5;i++) {
  EXPECT(spx_owner_string_from_utf8(context,invalid[i],invalid_len[i],&output)==3);
  EXPECT(memcmp(&output,&sentinel,sizeof(output))==0);
 }
 uint8_t lambda[]={0xce,0xbb}, large[4096];memset(large,'x',sizeof(large));
 const uint8_t *inputs[]={NULL,lambda,large};uint64_t lengths[]={0,2,4096};
 for(size_t i=0;i<3;i++) {
  source_address=(uintptr_t)inputs[i];
  EXPECT(spx_owner_string_from_utf8_signed(context,inputs[i],(int64_t)lengths[i],&output)==0);
  if(i==1) memset(lambda,'z',sizeof(lambda));
  if(i==2) memset(large,'z',sizeof(large));
  uint8_t matched=99;
  EXPECT(spx_owner_consume(context,output,(int64_t)lengths[i],&matched)==0);
  EXPECT(matched==1);
 }
 return 0;
}
