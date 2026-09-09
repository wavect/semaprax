static unsigned process_calls=0, process_settles=0;
static uint32_t package_run(void *context,uint64_t tool,spx_slice_u8_v1 argv,uint64_t length,
    spx_slice_u8_v1 input,uint64_t input_length,uint64_t timeout,uint64_t maxout,uint64_t maxerr,
    uint8_t *out,uint64_t capacity,uint64_t *written) {
    (void)context;
    const uint8_t expected[]={2,0,0,0,1,0,0,0,255,0,0,0,0};
    if(tool!=7 || length!=sizeof(expected) || argv.len<length || memcmp(argv.ptr,expected,sizeof(expected)) || input_length!=1 || input.len!=1 || input.ptr[0]!=66 || timeout!=100 || maxout!=2 || maxerr!=2 || capacity!=36) return 1;
    ++process_calls;
    memset(out,0,(size_t)capacity);
    out[0]=1;out[8]=252;out[9]=255;out[10]=255;out[11]=255;out[12]=3;
    out[16]=1;out[24]=1;out[32]=65;out[33]=33;*written=34;
    return 0;
}
static uint32_t package_settle(void *context){(void)context;++process_settles;return 0;}
int main(void){
    const struct spx_process_callbacks_v1 callbacks={NULL,package_run,package_settle};
    struct spx_language_command_input_v1 input={0};
    for(unsigned i=0;i<3;++i){
        struct spx_language_command_result_v1 result;
        if(spx_process_command_run_v1(&input,NULL,&callbacks,&result)!=1 || !result.semantic_success || !result.matched || result.stdout_length || result.stderr_length)return 1;
    }
    return process_calls==3 && process_settles==3 ? 0 : 2;
}
