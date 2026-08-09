//! Generate private Android callable-v3 providers and matching JNI C shims.

use std::error::Error;
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use semaprax::codegen::{
    emit_private_native_callable_v3_android_fixture, PrivateNativeCallableV3AndroidTarget,
    PrivateNativeCallableV3Fixture,
};
use semaprax::hir::DeclarationId;
use semaprax::owned_resource_corpus::build_owned_resource_corpus_v1;

fn main() -> Result<(), Box<dyn Error>> {
    let outputs = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if outputs.len() != 4
        || outputs.iter().any(|path| !path.is_absolute())
        || (0..outputs.len())
            .any(|left| (left + 1..outputs.len()).any(|right| outputs[left] == outputs[right]))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected four distinct absolute outputs: x86-provider.c arm64-provider.c x86-jni.c arm64-jni.c",
        )
        .into());
    }
    let corpus = build_owned_resource_corpus_v1()
        .map_err(|error| io::Error::other(format!("build owned corpus: {error:?}")))?;
    let emit = |target| {
        emit_private_native_callable_v3_android_fixture(
            &corpus.program,
            &DeclarationId::new("token.discard-two"),
            PrivateNativeCallableV3Fixture::ScalarDiscardTwo,
            target,
        )
        .map_err(|error| io::Error::other(format!("emit Android JNI fixture: {error:?}")))
    };
    let x86 = emit(PrivateNativeCallableV3AndroidTarget::X86_64)?;
    let arm64 = emit(PrivateNativeCallableV3AndroidTarget::Arm64)?;
    write_new(&outputs[0], render_provider(x86.source()).as_bytes())?;
    write_new(&outputs[1], render_provider(arm64.source()).as_bytes())?;
    write_new(&outputs[2], render_jni_shim(x86.descriptor()).as_bytes())?;
    write_new(&outputs[3], render_jni_shim(arm64.descriptor()).as_bytes())?;
    Ok(())
}

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.flush()
}

fn render_provider(provider: &str) -> String {
    format!(
        r#"{provider}

#include <stddef.h>
#include <stdint.h>

static _Thread_local uint32_t spx_jni_finalizer_count;
static _Thread_local uint32_t spx_jni_finalizer_owners[2];
static _Thread_local uint64_t spx_jni_finalizer_payloads[2];

static void spx_v3_generated_finalize(uint32_t owner,uint64_t payload){{
  uint32_t index=spx_jni_finalizer_count;
  if(index<UINT32_C(2)){{
    spx_jni_finalizer_owners[index]=owner;
    spx_jni_finalizer_payloads[index]=payload;
  }}
  spx_jni_finalizer_count=index+UINT32_C(1);
}}

__attribute__((visibility("default")))
uint32_t spx_private_android_jni_finalizer_reset_v1(void){{
  spx_jni_finalizer_count=UINT32_C(0);
  spx_jni_finalizer_owners[0]=spx_jni_finalizer_owners[1]=UINT32_C(0);
  spx_jni_finalizer_payloads[0]=spx_jni_finalizer_payloads[1]=UINT64_C(0);
  return UINT32_C(0);
}}

__attribute__((visibility("default")))
uint32_t spx_private_android_jni_finalizer_snapshot_v1(
  uint32_t *count,uint32_t *owners,uint64_t *payloads,uint32_t capacity
){{
  if(count==NULL||owners==NULL||payloads==NULL||capacity!=UINT32_C(2))return UINT32_C(1);
  *count=spx_jni_finalizer_count;
  owners[0]=spx_jni_finalizer_owners[0];owners[1]=spx_jni_finalizer_owners[1];
  payloads[0]=spx_jni_finalizer_payloads[0];payloads[1]=spx_jni_finalizer_payloads[1];
  return UINT32_C(0);
}}
"#,
    )
}

fn render_jni_shim(descriptor: &[u8]) -> String {
    let mut descriptor_bytes = String::new();
    for byte in descriptor {
        write!(descriptor_bytes, "0x{byte:02x},").expect("string writes are infallible");
    }
    format!(
        r#"#define _GNU_SOURCE 1
#include <dlfcn.h>
#include <jni.h>
#include <limits.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#define SPX_REENTRANT_STATUS UINT64_C(0x0000002d0000000b)
#define SPX_WRONG_THREAD_STATUS UINT64_C(0x0000002d00000002)
#define SPX_DECLARED_STATUS UINT64_C(0x0000006b00000007)
#define SPX_UNEXPECTED_STATUS UINT64_C(0x0000004500000001)
#define SPX_INVALID_STATUS UINT64_C(0x0000002d00000001)
#define SPX_HOOK_STATUS UINT64_C(0x0000002d80000004)

static const uint8_t spx_descriptor[]={{{descriptor_bytes}}};
struct spx_jni_evidence_v1 {{
  uint32_t size;uint32_t version;uint64_t module_instance_id;uint64_t proof_flags;
  uint64_t postcommit_allocations;uint64_t host_state_flags;
}};
_Static_assert(sizeof(struct spx_jni_evidence_v1)==40,"JNI evidence size");

extern uint64_t spx_private_android_jni_v1_open(const uint8_t *,uint32_t,const uint8_t *,uint32_t);
extern uint64_t spx_private_android_jni_v1_adopt_pair(uint64_t,uint64_t,uint64_t *);
extern uint64_t spx_private_android_jni_v1_consume_pair(uint64_t,struct spx_jni_evidence_v1 *,uint32_t);
extern uint64_t spx_private_android_jni_v1_poison_runtime(void);
extern uint64_t spx_private_android_jni_v1_validate_hooks(const void *,const void *);
extern uint64_t spx_private_android_jni_v1_close_runtime(void);

typedef uint32_t (*spx_reset_fn)(void);
typedef uint32_t (*spx_snapshot_fn)(uint32_t *,uint32_t *,uint64_t *,uint32_t);
static _Thread_local void *spx_hook_image;
static _Thread_local spx_reset_fn spx_reset;
static _Thread_local spx_snapshot_fn spx_snapshot;
static _Thread_local int spx_in_callback;

static jlong spx_open(JNIEnv *env,jobject self,jbyteArray path_array){{
  (void)self;
  if(spx_in_callback)return (jlong)SPX_REENTRANT_STATUS;
  if(path_array==NULL||spx_hook_image!=NULL)return (jlong)SPX_INVALID_STATUS;
  jsize length=(*env)->GetArrayLength(env,path_array);
  if((*env)->ExceptionCheck(env)){{(*env)->ExceptionClear(env);return (jlong)SPX_UNEXPECTED_STATUS;}}
  if(length<=0||length>4096)return (jlong)SPX_INVALID_STATUS;
  char path[4097];
  (*env)->GetByteArrayRegion(env,path_array,0,length,(jbyte *)path);
  if((*env)->ExceptionCheck(env)){{(*env)->ExceptionClear(env);return (jlong)SPX_UNEXPECTED_STATUS;}}
  if(memchr(path,'\0',(size_t)length)!=NULL)return (jlong)SPX_INVALID_STATUS;
  path[length]='\0';
  uint64_t status=spx_private_android_jni_v1_open((const uint8_t *)path,(uint32_t)length,spx_descriptor,(uint32_t)sizeof(spx_descriptor));
  if(status!=UINT64_C(0))return (jlong)status;
  spx_hook_image=dlopen(path,RTLD_NOW|RTLD_LOCAL);
  if(spx_hook_image==NULL){{(void)spx_private_android_jni_v1_close_runtime();return (jlong)SPX_HOOK_STATUS;}}
  void *reset_symbol=dlsym(spx_hook_image,"spx_private_android_jni_finalizer_reset_v1");
  void *snapshot_symbol=dlsym(spx_hook_image,"spx_private_android_jni_finalizer_snapshot_v1");
  _Static_assert(sizeof(reset_symbol)==sizeof(spx_reset),"reset function pointer representation");
  _Static_assert(sizeof(snapshot_symbol)==sizeof(spx_snapshot),"snapshot function pointer representation");
  memcpy(&spx_reset,&reset_symbol,sizeof(spx_reset));memcpy(&spx_snapshot,&snapshot_symbol,sizeof(spx_snapshot));
  Dl_info reset_info={{0}},snapshot_info={{0}};char canonical_path[PATH_MAX],reset_path[PATH_MAX],snapshot_path[PATH_MAX];
  void *reset_address=NULL,*snapshot_address=NULL;
  _Static_assert(sizeof(reset_address)==sizeof(spx_reset),"function pointer representation");
  _Static_assert(sizeof(snapshot_address)==sizeof(spx_snapshot),"function pointer representation");
  memcpy(&reset_address,&spx_reset,sizeof(reset_address));memcpy(&snapshot_address,&spx_snapshot,sizeof(snapshot_address));
  if(spx_reset==NULL||spx_snapshot==NULL||
     spx_private_android_jni_v1_validate_hooks(reset_address,snapshot_address)!=UINT64_C(0)||
     realpath(path,canonical_path)==NULL||
     dladdr(reset_address,&reset_info)==0||dladdr(snapshot_address,&snapshot_info)==0||
     reset_info.dli_fbase!=snapshot_info.dli_fbase||reset_info.dli_fname==NULL||snapshot_info.dli_fname==NULL||
     realpath(reset_info.dli_fname,reset_path)==NULL||realpath(snapshot_info.dli_fname,snapshot_path)==NULL||
     strcmp(canonical_path,reset_path)!=0||strcmp(canonical_path,snapshot_path)!=0||spx_reset()!=UINT32_C(0)){{
    (void)spx_private_android_jni_v1_close_runtime();(void)dlclose(spx_hook_image);
    spx_hook_image=NULL;spx_reset=NULL;spx_snapshot=NULL;return (jlong)SPX_HOOK_STATUS;
  }}
  return (jlong)UINT64_C(0);
}}

static jlong spx_adopt(JNIEnv *env,jobject self,jlong first,jlong second,jlongArray output){{
  (void)self;
  if(spx_in_callback)return (jlong)SPX_REENTRANT_STATUS;
  if(output==NULL)return (jlong)SPX_INVALID_STATUS;
  jsize length=(*env)->GetArrayLength(env,output);
  if((*env)->ExceptionCheck(env)){{(*env)->ExceptionClear(env);return (jlong)SPX_UNEXPECTED_STATUS;}}
  if(length<1)return (jlong)SPX_INVALID_STATUS;
  jboolean copied=JNI_FALSE;
  jlong *words=(*env)->GetLongArrayElements(env,output,&copied);
  if((*env)->ExceptionCheck(env)){{
    (*env)->ExceptionClear(env);if(words!=NULL)(*env)->ReleaseLongArrayElements(env,output,words,JNI_ABORT);
    return (jlong)SPX_UNEXPECTED_STATUS;
  }}
  if(words==NULL)return (jlong)SPX_UNEXPECTED_STATUS;
  uint64_t handle=UINT64_C(0);
  uint64_t status=spx_private_android_jni_v1_adopt_pair((uint64_t)first,(uint64_t)second,&handle);
  if(status==UINT64_C(0))words[0]=(jlong)handle;
  (*env)->ReleaseLongArrayElements(env,output,words,status==UINT64_C(0)?0:JNI_ABORT);
  return (jlong)status;
}}

static jlong spx_consume(JNIEnv *env,jobject self,jlong handle,jlongArray output){{
  (void)self;
  if(spx_in_callback)return (jlong)SPX_REENTRANT_STATUS;
  if(output==NULL)return (jlong)SPX_INVALID_STATUS;
  jsize length=(*env)->GetArrayLength(env,output);
  if((*env)->ExceptionCheck(env)){{(*env)->ExceptionClear(env);return (jlong)SPX_UNEXPECTED_STATUS;}}
  if(length<8)return (jlong)SPX_INVALID_STATUS;
  jboolean copied=JNI_FALSE;
  jlong *words=(*env)->GetLongArrayElements(env,output,&copied);
  if((*env)->ExceptionCheck(env)){{
    (*env)->ExceptionClear(env);if(words!=NULL)(*env)->ReleaseLongArrayElements(env,output,words,JNI_ABORT);
    return (jlong)SPX_UNEXPECTED_STATUS;
  }}
  if(words==NULL)return (jlong)SPX_UNEXPECTED_STATUS;
  if(spx_hook_image!=NULL&&(spx_reset==NULL||spx_reset()!=UINT32_C(0))){{
    (void)spx_private_android_jni_v1_poison_runtime();
    (*env)->ReleaseLongArrayElements(env,output,words,JNI_ABORT);return (jlong)SPX_HOOK_STATUS;
  }}
  struct spx_jni_evidence_v1 evidence={{0}};
  uint64_t status=spx_private_android_jni_v1_consume_pair((uint64_t)handle,&evidence,(uint32_t)sizeof(evidence));
  if(status==UINT64_C(0)){{
    uint32_t count=0,owners[2]={{0,0}};uint64_t payloads[2]={{0,0}};
    if(spx_snapshot==NULL||spx_snapshot(&count,owners,payloads,UINT32_C(2))!=UINT32_C(0)||
       evidence.size!=(uint32_t)sizeof(evidence)||evidence.version!=UINT32_C(1)||
       evidence.module_instance_id==UINT64_C(0)||evidence.proof_flags!=UINT64_C(0x0f)||
       evidence.postcommit_allocations!=UINT64_C(0)||evidence.host_state_flags!=UINT64_C(0)||
       count!=UINT32_C(2)||owners[0]!=UINT32_C(1)||payloads[0]!=UINT64_C(13)||
       owners[1]!=UINT32_C(0)||payloads[1]!=UINT64_C(11)){{
      (void)spx_private_android_jni_v1_poison_runtime();status=SPX_HOOK_STATUS;
    }}
    else{{
      words[0]=(jlong)UINT64_C(1);words[1]=(jlong)evidence.module_instance_id;
      words[2]=(jlong)evidence.proof_flags;words[3]=(jlong)evidence.postcommit_allocations;
      words[4]=(jlong)count;words[5]=(jlong)(((uint64_t)owners[0]<<32)|payloads[0]);
      words[6]=(jlong)(((uint64_t)owners[1]<<32)|payloads[1]);words[7]=(jlong)evidence.host_state_flags;
    }}
  }}
  (*env)->ReleaseLongArrayElements(env,output,words,status==UINT64_C(0)?0:JNI_ABORT);
  return (jlong)status;
}}

static jlong spx_close(JNIEnv *env,jobject self){{
  (void)env;(void)self;
  if(spx_in_callback)return (jlong)SPX_REENTRANT_STATUS;
  uint64_t status=spx_private_android_jni_v1_close_runtime();
  if(status==UINT64_C(0)&&spx_hook_image!=NULL){{
    (void)dlclose(spx_hook_image);spx_hook_image=NULL;spx_reset=NULL;spx_snapshot=NULL;
  }}
  return (jlong)status;
}}

static jlong spx_probe(JNIEnv *env,jobject self,jobject callback){{
  (void)self;
  if(spx_in_callback)return (jlong)SPX_REENTRANT_STATUS;
  if(spx_hook_image==NULL)return (jlong)SPX_WRONG_THREAD_STATUS;
  if(callback==NULL)return (jlong)SPX_INVALID_STATUS;
  jclass runnable=(*env)->GetObjectClass(env,callback);
  if(runnable==NULL||(*env)->ExceptionCheck(env)){{(*env)->ExceptionClear(env);return (jlong)SPX_UNEXPECTED_STATUS;}}
  jclass declared=(*env)->FindClass(env,"dev/semaprax/runtime/DeclaredFixtureException");
  if(declared==NULL||(*env)->ExceptionCheck(env)){{(*env)->ExceptionClear(env);return (jlong)SPX_UNEXPECTED_STATUS;}}
  jmethodID run=(*env)->GetMethodID(env,runnable,"run","()V");
  if(run==NULL||(*env)->ExceptionCheck(env)){{(*env)->ExceptionClear(env);return (jlong)SPX_UNEXPECTED_STATUS;}}
  spx_in_callback=1;
  (*env)->CallVoidMethod(env,callback,run);
  jthrowable thrown=NULL;
  if((*env)->ExceptionCheck(env)){{thrown=(*env)->ExceptionOccurred(env);(*env)->ExceptionClear(env);}}
  if(thrown==NULL){{spx_in_callback=0;return (jlong)UINT64_C(0);}}
  jboolean expected=(*env)->IsInstanceOf(env,thrown,declared);
  if((*env)->ExceptionCheck(env)){{(*env)->ExceptionClear(env);spx_in_callback=0;return (jlong)SPX_UNEXPECTED_STATUS;}}
  spx_in_callback=0;
  return (jlong)(expected?SPX_DECLARED_STATUS:SPX_UNEXPECTED_STATUS);
}}

static const JNINativeMethod spx_methods[]={{
  {{"nativeOpen","([B)J",(void *)spx_open}},
  {{"nativeAdoptPair","(JJ[J)J",(void *)spx_adopt}},
  {{"nativeConsume","(J[J)J",(void *)spx_consume}},
  {{"nativeCloseRuntime","()J",(void *)spx_close}},
  {{"nativeProbeException","(Ljava/lang/Runnable;)J",(void *)spx_probe}},
  {{"nativeConsumeRawWrongThread","(J[J)J",(void *)spx_consume}}
}};

JNIEXPORT jint JNICALL JNI_OnLoad(JavaVM *vm,void *reserved){{
  (void)reserved;JNIEnv *env=NULL;
  if((*vm)->GetEnv(vm,(void **)&env,JNI_VERSION_1_6)!=JNI_OK)return JNI_ERR;
  jclass bridge=(*env)->FindClass(env,"dev/semaprax/runtime/NativeBridge");
  if(bridge==NULL||(*env)->RegisterNatives(env,bridge,spx_methods,(jint)(sizeof(spx_methods)/sizeof(spx_methods[0])))!=JNI_OK)return JNI_ERR;
  return JNI_VERSION_1_6;
}}

JNIEXPORT void JNICALL JNI_OnUnload(JavaVM *vm,void *reserved){{(void)vm;(void)reserved;}}
"#,
    )
}

#[cfg(test)]
mod tests {
    use super::{render_jni_shim, render_provider};

    #[test]
    fn generated_units_freeze_hooks_registration_and_known_answers() {
        let provider = render_provider("/* provider */");
        assert!(provider.contains("spx_private_android_jni_finalizer_snapshot_v1"));
        assert!(provider.contains("spx_v3_generated_finalize"));
        assert!(provider.contains("static _Thread_local uint32_t spx_jni_finalizer_count"));
        let shim = render_jni_shim(&[0x53, 0x50, 0x58]);
        for required in [
            "0x53,0x50,0x58,",
            "dev/semaprax/runtime/NativeBridge",
            "dev/semaprax/runtime/DeclaredFixtureException",
            "nativeAdoptPair\",\"(JJ[J)J",
            "nativeConsume\",\"(J[J)J",
            "nativeProbeException",
            "SPX_REENTRANT_STATUS UINT64_C(0x0000002d0000000b)",
            "SPX_WRONG_THREAD_STATUS UINT64_C(0x0000002d00000002)",
            "SPX_UNEXPECTED_STATUS UINT64_C(0x0000004500000001)",
            "reset_info.dli_fbase!=snapshot_info.dli_fbase",
            "strcmp(canonical_path,reset_path)!=0",
            "spx_in_callback=1",
            "spx_in_callback=0",
            "evidence.size!=(uint32_t)sizeof(evidence)",
            "owners[0]!=UINT32_C(1)||payloads[0]!=UINT64_C(13)",
            "GetLongArrayElements",
            "status==UINT64_C(0)?0:JNI_ABORT",
            "spx_private_android_jni_v1_poison_runtime()",
            "spx_hook_image!=NULL&&(spx_reset==NULL||spx_reset()!=UINT32_C(0))",
            "memcpy(&spx_reset,&reset_symbol,sizeof(spx_reset))",
            "spx_private_android_jni_v1_validate_hooks(reset_address,snapshot_address)",
        ] {
            assert!(shim.contains(required), "missing `{required}`");
        }
    }
}
