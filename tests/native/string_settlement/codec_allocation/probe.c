/* POSIX subprocesses observe the actual unmodified fatal runtime. No signal
 * handler, longjmp, substituted runtime helper, or forced owner cleanup. */
#include <errno.h>
#include <signal.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>

static uint8_t *fault_borrowed_input;
static spx_slice_u8_v1 input_view(void) {
    return (spx_slice_u8_v1){.ptr=fault_borrowed_input,.len=sizeof fault_input};
}
static void context_init(struct spx_context *context, struct spx_status_entry *entries) {
    REQUIRE(spx_context_init(context, 74, entries, 16, NULL, NULL, NULL));
}
static size_t baseline(bool composed) {
    struct spx_status_entry entries[16]; struct spx_context context={0};
    context_init(&context,entries);
    fault_attempts=0; fault_selected=0;
    int64_t result=INT64_MIN;
    spx_status_token status=composed ? FAULT_COMPOSE(&context,input_view(),&result)
                                    : FAULT_DECODE(&context,input_view(),&result);
    REQUIRE(status==SPX_STATUS_SUCCESS && result==724);
    REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
    REQUIRE(context.call_depth==0 && context.borrowed_str_depth==0);
    /* A bounded fixture census, not a new language allocation allowance. */
    REQUIRE(fault_attempts>0 && fault_attempts<=4096);
    return fault_attempts;
}
static void child(size_t selected,int64_t *publication) {
    struct rlimit cores={0,0}; REQUIRE(setrlimit(RLIMIT_CORE,&cores)==0);
    struct spx_status_entry entries[16]; struct spx_context context={0};
    context_init(&context,entries);
    fault_attempts=0;fault_selected=selected;
    spx_status_token status=FAULT_COMPOSE(&context,input_view(),publication);
    /* Reaching here must be a recoverable Vec allocation refusal, selected
     * before cleanup. String malloc failure instead aborts in the real runtime. */
    REQUIRE(status!=SPX_STATUS_SUCCESS && fault_attempts==selected);
    const struct spx_normalized_status *normalized=spx_status_resolve(&context,status);
    REQUIRE(normalized!=NULL && !strcmp(normalized->domain_id,"semaprax.vec.v1"));
    REQUIRE(normalized->code==3 && *publication==INT64_MIN);
    REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
    for(size_t slot=0;slot<512;++slot)REQUIRE(fixture_table[slot].pointer==NULL);
    REQUIRE(context.call_depth==0 && context.borrowed_str_depth==0);
    REQUIRE(context.status_arena.length==1);
    /* Recover with the same live invocation context; do not reset its arena or
     * production budget. Only the external one-shot injection is disarmed. */
    fault_selected=0;
    int64_t recovered=INT64_MIN;
    REQUIRE(FAULT_COMPOSE(&context,input_view(),&recovered)==SPX_STATUS_SUCCESS);
    REQUIRE(recovered==724 && context.status_arena.length==1);
    REQUIRE(spx_status_resolve(&context,status)==normalized);
    REQUIRE(normalized->code==3 && !strcmp(normalized->domain_id,"semaprax.vec.v1"));
    REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
    REQUIRE(context.call_depth==0 && context.borrowed_str_depth==0);
    _exit(42);
}
int main(void) {
    REQUIRE(fixture_binary_stdout());
    /* The actual borrowed bytes live in a shared read-only mapping. A child
     * write cannot hide behind fork's private copy-on-write globals, including
     * at fatal sites: any such write yields a disallowed signal. */
    fault_borrowed_input=mmap(NULL,sizeof fault_input,PROT_READ|PROT_WRITE,
                              MAP_SHARED|MAP_ANONYMOUS,-1,0);
    REQUIRE(fault_borrowed_input!=MAP_FAILED);
    memcpy(fault_borrowed_input,fault_input,sizeof fault_input);
    REQUIRE(mprotect(fault_borrowed_input,sizeof fault_input,PROT_READ)==0);
    size_t decoded=baseline(false),composed=baseline(true);
    REQUIRE(composed>decoded);
    int64_t *publication=mmap(NULL,sizeof(*publication),PROT_READ|PROT_WRITE,
                              MAP_SHARED|MAP_ANONYMOUS,-1,0);
    REQUIRE(publication!=MAP_FAILED);
    size_t returned=0,fatal_decode=0,fatal_encode=0;
    for(unsigned repetition=0;repetition<2;++repetition){
        for(size_t selected=1;selected<=composed;++selected){
            int errors[2];REQUIRE(pipe(errors)==0);*publication=INT64_MIN;
            pid_t pid=fork();REQUIRE(pid>=0);
            if(pid==0){
                REQUIRE(close(errors[0])==0);
                REQUIRE(dup2(errors[1],STDERR_FILENO)==STDERR_FILENO);
                REQUIRE(close(errors[1])==0);
                child(selected,publication);
            }
            REQUIRE(close(errors[1])==0);
            char message[256];size_t length=0;
            for(;;){
                REQUIRE(length<sizeof(message)-1);
                ssize_t n=read(errors[0],message+length,sizeof(message)-1-length);
                if(n<0 && errno==EINTR)continue;
                REQUIRE(n>=0);if(n==0)break;length+=(size_t)n;
            }
            message[length]='\0';REQUIRE(close(errors[0])==0);
            int status;pid_t waited;
            do{waited=waitpid(pid,&status,0);}while(waited<0 && errno==EINTR);
            REQUIRE(waited==pid && *publication==INT64_MIN);
            if(WIFEXITED(status)){
                REQUIRE(WEXITSTATUS(status)==42 && length==0);++returned;
            }else{
                REQUIRE(WIFSIGNALED(status) && WTERMSIG(status)==SIGABRT);
                REQUIRE(!strcmp(message,"SEMAPRAX native runtime invariant failure: string allocation failed\n"));
                if(selected<=decoded)++fatal_decode;else ++fatal_encode;
                /* No claim about live-owner cleanup at these fatal sites. */
            }
            REQUIRE(!memcmp(fault_borrowed_input,fault_input,sizeof fault_input));
        }
    }
    REQUIRE(returned>0 && fatal_decode>0 && fatal_encode>0);
    REQUIRE(returned+fatal_decode+fatal_encode==2*composed);
    REQUIRE(munmap(publication,sizeof(*publication))==0);
    /* Parent inventory is unchanged by subprocess fault observation. */
    REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
    REQUIRE(baseline(true)==composed);
    REQUIRE(!memcmp(fault_borrowed_input,fault_input,sizeof fault_input));
    REQUIRE(munmap(fault_borrowed_input,sizeof fault_input)==0);
    (void)puts("native-ordinary-strings-settled");return 0;
}
