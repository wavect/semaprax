#undef malloc
#undef calloc
#undef free
static int mode=0;
static unsigned runs=0,settlements=0;
static uint32_t run(void *context,uint64_t tool,spx_slice_u8_v1 argv,uint64_t argc,
    spx_slice_u8_v1 input,uint64_t input_length,uint64_t timeout,uint64_t maxout,uint64_t maxerr,
    uint8_t *out,uint64_t capacity,uint64_t *written) {
    (void)context; ++runs;
    assert(tool==7 && argc==4 && argv.len==4 && input.len==0 && input_length==0);
    assert(timeout==100 && maxout==2 && maxerr==2 && capacity==36);
    for(unsigned i=0;i<4;++i)assert(argv.ptr[i]==0);
    if(mode==2 || mode==4)return 4;
    if(mode==99)return 99;
    if(mode>=10 && mode<=16)return (uint32_t)(mode-9);
    if(mode==6)return 0; /* poisoned result length must remain rejected */
    memset(out,0,(size_t)capacity);
    out[0]=mode==5?2:1; out[8]=28; out[16]=1; out[24]=1; out[32]=65;out[33]=33;
    *written=mode==7?37:34;
    if(mode==8){out[16]=3;out[24]=0;*written=35;}
    return 0;
}
static uint32_t settle(void *context){(void)context;++settlements;return mode==3||mode==4?7:0;}
static int invoke(int selected,uint32_t expected) {
    mode=selected;runs=0;settlements=0;
    struct spx_language_command_input_v1 input={0};
    struct spx_language_command_result_v1 output;
    const struct spx_process_callbacks_v1 callbacks={NULL,run,settle};
    memset(&output,0x5a,sizeof(output));
    if(spx_process_command_run_v1(&input,NULL,&callbacks,&output)!=1)return 10;
    if(runs!=1||settlements!=1||process_live_allocations!=0)return 11;
    if(expected==0){
        if(!output.semantic_success||!output.matched||output.stdout_length!=4||output.stderr_length!=4)return 12;
        for(unsigned i=0;i<4;++i)if(output.stdout_bytes[i]!=0||output.stderr_bytes[i]!=0)return 13;
    } else if(output.semantic_success||output.stdout_length||output.stderr_length||output.status_code!=expected||strcmp(output.status_domain,"semaprax.process.v1"))return 14;
    return 0;
}
int main(int argc,char **argv){
    (void)argv;if(argc>1)return invoke(99,0);
    for(int repeat=0;repeat<2;++repeat){
        const int modes[]={0,2,3,4,5,6,7,8,10,11,12,13,14,15,16};const uint32_t codes[]={0,4,7,4,6,6,5,5,1,2,3,4,5,6,7};
        for(unsigned i=0;i<sizeof(modes)/sizeof(modes[0]);++i){int code=invoke(modes[i],codes[i]);if(code)return code;if(invoke(0,0))return 15;}
    }
    return 0;
}
