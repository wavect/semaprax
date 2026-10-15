/* Trusted runtime boundary only: source creates the real vector and leaves.
   Raw float patterns below exercise the existing scalar-word ABI, not a new
   source intrinsic or an untyped owning handle. */
#undef malloc
#undef calloc
#undef realloc
#undef free
#undef abort
static struct spx_context probe_context;
static struct spx_status_entry probe_entries[32];
static spx_vec_v1 probe_owner, probe_input, probe_output;
static spx_leaf_layout_v1 probe_forged;
static unsigned char probe_prior[80];
static const uint64_t probe_f32[] = {
 UINT64_C(0),UINT64_C(0x80000000),UINT64_C(0x7f800000),UINT64_C(0xff800000),
 UINT64_C(0x7fc00001),UINT64_C(0x7fc12345),UINT64_C(0xffc54321),
 UINT64_C(1),UINT64_C(0x007fffff),UINT64_C(0x00800000),UINT64_C(0x7f7fffff),UINT64_C(0xff7fffff)
};
static const uint64_t probe_f64[] = {
 UINT64_C(0),UINT64_C(0x8000000000000000),UINT64_C(0x7ff0000000000000),UINT64_C(0xfff0000000000000),
 UINT64_C(0x7ff8000000000001),UINT64_C(0x7ff8123456789abc),UINT64_C(0xfff8fedcba987654),
 UINT64_C(1),UINT64_C(0x000fffffffffffff),UINT64_C(0x0010000000000000),UINT64_C(0x7fefffffffffffff),UINT64_C(0xffefffffffffffff)
};
static void probe_seed(void) {
 if(spx_decl_6669656c642e73656564(&probe_context,&probe_owner)!=SPX_STATUS_SUCCESS)abort();
 if(probe_owner.len!=1||probe_owner.capacity!=1)abort();
}
static void probe_drop(void) {
 spx_vec_drop(&probe_context,&probe_owner);
 if(fixture_live)abort();
 for(uint32_t k=0;k<SPX_VEC_AUTHORITY_CAPACITY;++k)if(probe_context.vec_authority[k].live)abort();
}
int main(void) {
 if(!spx_context_init(&probe_context,UINT64_C(47),probe_entries,32,NULL,NULL,NULL))return 1;
 const spx_leaf_layout_v1 *d=&PROBE_LAYOUT;
 if(d->count!=6||d->kinds[1]!=PROBE_KIND||d->kinds[4]!=6||d->kinds[5]!=7)return 2;
 for(unsigned repeat=0;repeat<3;++repeat) {
  probe_seed();
  for(unsigned k=0;k<12;++k) {
   unsigned char *row=(unsigned char *)(void *)probe_owner.ptr;
   uint64_t other32=probe_f32[(k+5)%12],other64=probe_f64[(k+3)%12];
   memcpy(row+d->offsets[4],&other32,8);memcpy(row+d->offsets[5],&other64,8);
   uint64_t marker=UINT64_C(0x8000000000000000)+k;memcpy(row+d->offsets[3],&marker,8);
   memcpy(probe_prior,row,d->stride);
   uint64_t word=PROBE_KIND==6?probe_f32[k]:probe_f64[k];
   uint64_t next=probe_context.vec_next_generation;spx_vec_v1 before=probe_owner;
   fixture_sort_active=1;
   spx_status_token status=spx_leaf_update_field(&probe_context,&probe_owner,d,0,1,PROBE_KIND,word,&probe_output);
   fixture_sort_active=0;
   if(status!=SPX_STATUS_SUCCESS||probe_owner.generation||probe_output.generation!=next
    ||probe_context.vec_next_generation!=next+1||probe_output.ptr!=before.ptr||probe_output.authority!=before.authority
    ||probe_output.capacity!=before.capacity||probe_output.len!=before.len)return 3;
   uint64_t actual=0;memcpy(&actual,row+d->offsets[1],8);
   if(actual!=word||memcmp(probe_prior,row,d->offsets[1])
    ||memcmp(probe_prior+d->offsets[1]+8,row+d->offsets[1]+8,d->stride-d->offsets[1]-8))return 4;
   probe_owner=probe_output;probe_output=(spx_vec_v1){0};
  }
  probe_drop();
 }
 /* Each hostile attempt has a fresh ordinary source-created owner. Catch only
    the generated invariant fail-stop, then prove no slot/owner/result changed
    before settling the original valid owner through the real runtime. */
 for(unsigned attack=0;attack<8;++attack) {
  probe_seed();probe_input=probe_owner;memset(&probe_output,0xa5,sizeof(probe_output));
  unsigned char *row=(unsigned char *)(void *)probe_owner.ptr;
  memcpy(probe_prior,row,d->stride);
  const spx_leaf_layout_v1 *candidate=d;uint32_t field=1,kind=PROBE_KIND;
  uint64_t next=probe_context.vec_next_generation;
  if(attack==0)probe_input.generation^=UINT64_C(1);
  if(attack==1){probe_forged=*d;probe_forged.identity="foreign.nominal";candidate=&probe_forged;}
  if(attack==2)probe_input.authority=0;
  if(attack==3)probe_input.ptr=(spx_bytes_v1 *)(void *)(row+8);
  if(attack==4)++probe_input.len;
  if(attack==5)probe_context.vec_next_generation=UINT64_MAX;
  if(attack==6){field=0;kind=9;}
  if(attack==7)kind=kind==6?7:6;
  spx_vec_v1 before_input=probe_input,before_output=probe_output;
  uint64_t expected_next=probe_context.vec_next_generation;
  unsigned live=fixture_live;uint32_t statuses=probe_context.status_arena.length;
  fixture_expect_trap=1;fixture_sort_active=1;
  if(setjmp(fixture_trap)==0) {
   (void)spx_leaf_update_field(&probe_context,&probe_input,candidate,0,field,kind,0,&probe_output);
   return 5;
  }
  fixture_expect_trap=0;fixture_sort_active=0;
  if(memcmp(&probe_input,&before_input,sizeof(probe_input))||memcmp(&probe_output,&before_output,sizeof(probe_output))
   ||memcmp(row,probe_prior,d->stride)||fixture_live!=live||probe_context.vec_next_generation!=expected_next
   ||probe_context.status_arena.length!=statuses)return 6;
  probe_context.vec_next_generation=next;
  probe_drop();
 }
 return 0;
}
