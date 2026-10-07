/* Typed Map/Set v2: closed atoms; borrowed Strings copied into private storage. */
typedef union spx_map_atom_v2 { char *text; int64_t i64; int32_t i32; uint8_t u8; uint64_t usize; uint32_t character; float f32; double f64; bool boolean; } spx_map_atom_v2;
typedef struct spx_map_entry_v2 { spx_map_atom_v2 key, value; } spx_map_entry_v2;
typedef struct spx_map_v2 { uint64_t len, capacity, allocated; uint32_t key_tag, value_tag; spx_map_entry_v2 *entries; } spx_map_v2;
static __attribute__((unused)) spx_status_token spx_collection_failure_v2(struct spx_context *ctx, uint32_t code) {
    spx_status_token token=SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(ctx,"semaprax.map.v2",code,SPX_STATUS_CLASS_ADAPTER,SPX_RETRYABILITY_FALSE,&token)) spx_runtime_invariant_failure("collection status unavailable");
    return token;
}
static __attribute__((unused)) spx_map_atom_v2 spx_collection_copy_v2(uint32_t tag, spx_map_atom_v2 atom) {
    if (tag==1) atom.text=spx_string_from_literal(atom.text,spx_string_length_v10(atom.text));
    return atom;
}
static __attribute__((unused)) void spx_collection_atom_drop_v2(uint32_t tag, spx_map_atom_v2 atom) { if (tag==1) spx_string_drop(atom.text); }
static __attribute__((unused)) int spx_collection_order_v2(uint32_t tag, spx_map_atom_v2 a, spx_map_atom_v2 b) {
    if (tag==1) {uint64_t x=spx_string_length_v10(a.text),y=spx_string_length_v10(b.text),n=x<y?x:y;int order=n==0?0:memcmp(a.text,b.text,(size_t)n);return order!=0?(order<0?-1:1):(x<y?-1:(x>y?1:0));}
    if (tag==2) return a.i64<b.i64?-1:(a.i64>b.i64?1:0);
    if (tag==3) return a.boolean==b.boolean?0:(a.boolean?1:-1);
    spx_runtime_invariant_failure("invalid collection key tag");return 0;
}
static __attribute__((unused)) bool spx_collection_find_v2(const spx_map_v2 *map, spx_map_atom_v2 key, uint64_t *out) {
    uint64_t lo=0,hi=map->len;while(lo<hi){uint64_t mid=lo+(hi-lo)/2;int c=spx_collection_order_v2(map->key_tag,map->entries[mid].key,key);if(c==0){if(out)*out=mid;return true;}if(c<0)lo=mid+1;else hi=mid;}if(out)*out=lo;return false;
}
static __attribute__((unused)) spx_status_token spx_collection_new_v2(struct spx_context *ctx,uint32_t kt,uint32_t vt,uint64_t capacity,spx_map_v2 **out) {
    if(capacity>UINT64_C(65536))return spx_collection_failure_v2(ctx,3);
    spx_map_v2 *map=(spx_map_v2*)calloc(1,sizeof(*map));if(!map)spx_runtime_invariant_failure("collection allocation failed");map->capacity=capacity;map->key_tag=kt;map->value_tag=vt;*out=map;return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) spx_status_token spx_collection_put_v2(struct spx_context *ctx,spx_map_v2 *map,spx_map_atom_v2 key,spx_map_atom_v2 value,bool add,spx_map_v2 **out) {
    uint64_t index=0;if(spx_collection_find_v2(map,key,&index)){
        if(add){int64_t old=map->entries[index].value.i64,v=value.i64;if((v>0&&old>INT64_MAX-v)||(v<0&&old<INT64_MIN-v))return spx_collection_failure_v2(ctx,4);value.i64=old+v;}
        spx_map_atom_v2 copy=spx_collection_copy_v2(map->value_tag,value);spx_collection_atom_drop_v2(map->value_tag,map->entries[index].value);map->entries[index].value=copy;*out=map;return SPX_STATUS_SUCCESS;
    }
    if(map->len==map->capacity)return spx_collection_failure_v2(ctx,1);
    if(map->len==map->allocated){uint64_t grown=map->allocated==0?8:map->allocated*2;if(grown>map->capacity)grown=map->capacity;spx_map_entry_v2 *entries=(spx_map_entry_v2*)malloc((size_t)grown*sizeof(*entries));if(!entries)spx_runtime_invariant_failure("collection allocation failed");if(map->len)memcpy(entries,map->entries,(size_t)map->len*sizeof(*entries));free(map->entries);map->entries=entries;map->allocated=grown;}
    spx_map_atom_v2 key_copy=spx_collection_copy_v2(map->key_tag,key),value_copy=spx_collection_copy_v2(map->value_tag,value);
    if(index<map->len)memmove(map->entries+index+1,map->entries+index,(size_t)(map->len-index)*sizeof(*map->entries));map->entries[index].key=key_copy;map->entries[index].value=value_copy;map->len++;*out=map;return SPX_STATUS_SUCCESS;
}
static __attribute__((unused)) spx_map_v2 *spx_collection_remove_v2(spx_map_v2 *map,spx_map_atom_v2 key) {
    uint64_t i=0;if(!spx_collection_find_v2(map,key,&i))return map;spx_collection_atom_drop_v2(map->key_tag,map->entries[i].key);spx_collection_atom_drop_v2(map->value_tag,map->entries[i].value);if(i+1<map->len)memmove(map->entries+i,map->entries+i+1,(size_t)(map->len-i-1)*sizeof(*map->entries));map->len--;memset(map->entries+map->len,0,sizeof(*map->entries));return map;
}
static __attribute__((unused)) spx_map_atom_v2 spx_collection_get_v2(const spx_map_v2 *map,spx_map_atom_v2 key,spx_map_atom_v2 fallback) {uint64_t i=0;return spx_collection_copy_v2(map->value_tag,spx_collection_find_v2(map,key,&i)?map->entries[i].value:fallback);}
static __attribute__((unused)) spx_status_token spx_collection_at_v2(struct spx_context *ctx,const spx_map_v2 *map,uint64_t index,bool key,spx_map_atom_v2 *out) {if(index>=map->len)return spx_collection_failure_v2(ctx,2);*out=spx_collection_copy_v2(key?map->key_tag:map->value_tag,key?map->entries[index].key:map->entries[index].value);return SPX_STATUS_SUCCESS;}
static __attribute__((unused)) void spx_map_drop_v2(spx_map_v2 *map) {if(!map)return;for(uint64_t i=0;i<map->len;i++){spx_collection_atom_drop_v2(map->key_tag,map->entries[i].key);spx_collection_atom_drop_v2(map->value_tag,map->entries[i].value);}free(map->entries);free(map);}
