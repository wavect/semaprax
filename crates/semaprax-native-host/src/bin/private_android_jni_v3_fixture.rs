//! Generate private Android callable-v3 providers and matching JNI C shims.

use std::error::Error;
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use semaprax::codegen::{
    emit_private_native_callable_v3_android_corpus_fixture,
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
    if outputs.len() != 8
        || outputs.iter().any(|path| !path.is_absolute())
        || (0..outputs.len())
            .any(|left| (left + 1..outputs.len()).any(|right| outputs[left] == outputs[right]))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected eight distinct absolute outputs: x86-discard.c arm64-discard.c \
             x86-requires-false.c arm64-requires-false.c x86-identity-max.c \
             arm64-identity-max.c x86-jni.c arm64-jni.c",
        )
        .into());
    }
    let corpus = build_owned_resource_corpus_v1()
        .map_err(|error| io::Error::other(format!("build owned corpus: {error:?}")))?;
    let requires_false = corpus
        .cases
        .iter()
        .find(|case| case.scenario_id == "requires-false")
        .ok_or_else(|| io::Error::other("requires-false corpus case is absent"))?;
    let requires_false_function = DeclarationId::new(requires_false.function_id);
    let identity_max = corpus
        .cases
        .iter()
        .find(|case| case.scenario_id == "identity-max")
        .ok_or_else(|| io::Error::other("identity-max corpus case is absent"))?;
    if identity_max.expected_owned_result_ordinal != Some(0) {
        return Err(io::Error::other("identity-max corpus result ordinal diverged").into());
    }
    let identity_max_function = DeclarationId::new(identity_max.function_id);
    let emit = |target| {
        emit_private_native_callable_v3_android_fixture(
            &corpus.program,
            &DeclarationId::new("token.discard-two"),
            PrivateNativeCallableV3Fixture::ScalarDiscardTwo,
            target,
        )
        .map_err(|error| io::Error::other(format!("emit Android JNI fixture: {error:?}")))
    };
    let emit_requires_false = |target| {
        emit_private_native_callable_v3_android_corpus_fixture(
            &corpus.program,
            &requires_false_function,
            &requires_false.arguments,
            requires_false.expected_owned_result_ordinal,
            &requires_false.reference,
            target,
        )
        .map_err(|error| {
            io::Error::other(format!(
                "emit Android JNI requires-false fixture: {error:?}"
            ))
        })
    };
    let emit_identity_max = |target| {
        emit_private_native_callable_v3_android_corpus_fixture(
            &corpus.program,
            &identity_max_function,
            &identity_max.arguments,
            identity_max.expected_owned_result_ordinal,
            &identity_max.reference,
            target,
        )
        .map_err(|error| {
            io::Error::other(format!("emit Android JNI identity-max fixture: {error:?}"))
        })
    };
    let x86 = emit(PrivateNativeCallableV3AndroidTarget::X86_64)?;
    let arm64 = emit(PrivateNativeCallableV3AndroidTarget::Arm64)?;
    let x86_requires_false = emit_requires_false(PrivateNativeCallableV3AndroidTarget::X86_64)?;
    let arm64_requires_false = emit_requires_false(PrivateNativeCallableV3AndroidTarget::Arm64)?;
    let x86_identity_max = emit_identity_max(PrivateNativeCallableV3AndroidTarget::X86_64)?;
    let arm64_identity_max = emit_identity_max(PrivateNativeCallableV3AndroidTarget::Arm64)?;
    write_new(&outputs[0], render_provider(x86.source()).as_bytes())?;
    write_new(&outputs[1], render_provider(arm64.source()).as_bytes())?;
    write_new(
        &outputs[2],
        render_provider(x86_requires_false.source()).as_bytes(),
    )?;
    write_new(
        &outputs[3],
        render_provider(arm64_requires_false.source()).as_bytes(),
    )?;
    write_new(
        &outputs[4],
        render_provider(x86_identity_max.source()).as_bytes(),
    )?;
    write_new(
        &outputs[5],
        render_provider(arm64_identity_max.source()).as_bytes(),
    )?;
    write_new(
        &outputs[6],
        render_jni_shim(
            x86.descriptor(),
            x86_requires_false.descriptor(),
            x86_identity_max.descriptor(),
        )
        .as_bytes(),
    )?;
    write_new(
        &outputs[7],
        render_jni_shim(
            arm64.descriptor(),
            arm64_requires_false.descriptor(),
            arm64_identity_max.descriptor(),
        )
        .as_bytes(),
    )?;
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

fn descriptor_literal(descriptor: &[u8]) -> String {
    let mut descriptor_bytes = String::new();
    for byte in descriptor {
        write!(descriptor_bytes, "0x{byte:02x},").expect("string writes are infallible");
    }
    descriptor_bytes
}

fn render_jni_shim(
    discard_descriptor: &[u8],
    requires_false_descriptor: &[u8],
    identity_max_descriptor: &[u8],
) -> String {
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
#define SPX_SELECTOR_DISCARD INT32_C(0)
#define SPX_SELECTOR_REQUIRES_FALSE INT32_C(1)
#define SPX_SELECTOR_IDENTITY_MAX INT32_C(2)
#define SPX_REQUIRES_FALSE_SELECTED_ORDINAL UINT64_C(1)
#define SPX_REQUIRES_FALSE_PAYLOAD UINT64_C(18446744073709551615)
#define SPX_IDENTITY_MAX_PUBLICATIONS UINT64_C(2)

static const uint8_t spx_descriptor[]={{{}}};
static const uint8_t spx_rf_descriptor[]={{{}}};
static const uint8_t spx_id_descriptor[]={{{}}};
struct spx_jni_evidence_v1 {{
  uint32_t size;uint32_t version;uint64_t module_instance_id;uint64_t proof_flags;
  uint64_t postcommit_allocations;uint64_t host_state_flags;
}};
_Static_assert(sizeof(struct spx_jni_evidence_v1)==40,"JNI evidence size");

extern uint64_t spx_private_android_jni_v1_open(const uint8_t *,uint32_t,const uint8_t *,uint32_t);
extern uint64_t spx_private_android_jni_v1_adopt_pair(uint64_t,uint64_t,uint64_t *);
extern uint64_t spx_private_android_jni_v1_adopt_single(uint64_t,uint64_t *);
extern uint64_t spx_private_android_jni_v1_adopt_owned(uint64_t,uint64_t *);
extern uint64_t spx_private_android_jni_v1_consume_pair(uint64_t,struct spx_jni_evidence_v1 *,uint32_t);
extern uint64_t spx_private_android_jni_v1_execute_requires_false(uint64_t,struct spx_jni_evidence_v1 *,uint32_t);
extern uint64_t spx_private_android_jni_v1_execute_identity_max(uint64_t,struct spx_jni_evidence_v1 *,uint32_t);
extern uint64_t spx_private_android_jni_v1_poison_runtime(void);
extern uint64_t spx_private_android_jni_v1_validate_hooks(const void *,const void *);
extern uint64_t spx_private_android_jni_v1_close_runtime(void);

typedef uint32_t (*spx_reset_fn)(void);
typedef uint32_t (*spx_snapshot_fn)(uint32_t *,uint32_t *,uint64_t *,uint32_t);
static _Thread_local void *spx_hook_image;
static _Thread_local spx_reset_fn spx_reset;
static _Thread_local spx_snapshot_fn spx_snapshot;
static _Thread_local int spx_in_callback;
static _Thread_local int spx_selector;

static jlong spx_open(JNIEnv *env,jobject self,jbyteArray path_array,jint selector){{
  (void)self;
  if(spx_in_callback)return (jlong)SPX_REENTRANT_STATUS;
  if(path_array==NULL||spx_hook_image!=NULL)return (jlong)SPX_INVALID_STATUS;
  if(selector!=SPX_SELECTOR_DISCARD&&selector!=SPX_SELECTOR_REQUIRES_FALSE&&selector!=SPX_SELECTOR_IDENTITY_MAX)return (jlong)SPX_INVALID_STATUS;
  const uint8_t *embedded_descriptor=spx_descriptor;
  uint32_t embedded_length=(uint32_t)sizeof(spx_descriptor);
  if(selector==SPX_SELECTOR_REQUIRES_FALSE){{
    embedded_descriptor=spx_rf_descriptor;
    embedded_length=(uint32_t)sizeof(spx_rf_descriptor);
  }}
  if(selector==SPX_SELECTOR_IDENTITY_MAX){{
    embedded_descriptor=spx_id_descriptor;
    embedded_length=(uint32_t)sizeof(spx_id_descriptor);
  }}
  jsize length=(*env)->GetArrayLength(env,path_array);
  if((*env)->ExceptionCheck(env)){{(*env)->ExceptionClear(env);return (jlong)SPX_UNEXPECTED_STATUS;}}
  if(length<=0||length>4096)return (jlong)SPX_INVALID_STATUS;
  char path[4097];
  (*env)->GetByteArrayRegion(env,path_array,0,length,(jbyte *)path);
  if((*env)->ExceptionCheck(env)){{(*env)->ExceptionClear(env);return (jlong)SPX_UNEXPECTED_STATUS;}}
  if(memchr(path,'\0',(size_t)length)!=NULL)return (jlong)SPX_INVALID_STATUS;
  path[length]='\0';
  uint64_t status=spx_private_android_jni_v1_open((const uint8_t *)path,(uint32_t)length,embedded_descriptor,embedded_length);
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
  spx_selector=(int)selector;
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

static jlong spx_adopt_single(JNIEnv *env,jobject self,jlong payload,jlongArray output){{
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
   uint64_t status=spx_private_android_jni_v1_adopt_single((uint64_t)payload,&handle);
   if(status==UINT64_C(0))words[0]=(jlong)handle;
   (*env)->ReleaseLongArrayElements(env,output,words,status==UINT64_C(0)?0:JNI_ABORT);
   return (jlong)status;
 }}

static jlong spx_adopt_owned(JNIEnv *env,jobject self,jlong payload,jlongArray output){{
  (void)self;
  if(spx_in_callback)return (jlong)SPX_REENTRANT_STATUS;
  if(spx_selector!=SPX_SELECTOR_IDENTITY_MAX)return (jlong)SPX_INVALID_STATUS;
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
  uint64_t status=spx_private_android_jni_v1_adopt_owned((uint64_t)payload,&handle);
  if(status==UINT64_C(0))words[0]=(jlong)handle;
  (*env)->ReleaseLongArrayElements(env,output,words,status==UINT64_C(0)?0:JNI_ABORT);
  return (jlong)status;
 }}

static jlong spx_consume(JNIEnv *env,jobject self,jlong handle,jlongArray output){{
  (void)self;
  if(spx_in_callback)return (jlong)SPX_REENTRANT_STATUS;
  if(spx_selector!=SPX_SELECTOR_DISCARD)return (jlong)SPX_INVALID_STATUS;
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

static jlong spx_execute_requires_false(JNIEnv *env,jobject self,jlong handle,jlongArray output){{
  (void)self;
  if(spx_in_callback)return (jlong)SPX_REENTRANT_STATUS;
  if(spx_selector!=SPX_SELECTOR_REQUIRES_FALSE)return (jlong)SPX_INVALID_STATUS;
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
  uint64_t status=spx_private_android_jni_v1_execute_requires_false((uint64_t)handle,&evidence,(uint32_t)sizeof(evidence));
  if(status==UINT64_C(0)){{
    uint32_t count=0,owners[2]={{0,0}};uint64_t payloads[2]={{0,0}};
    if(spx_snapshot==NULL||spx_snapshot(&count,owners,payloads,UINT32_C(2))!=UINT32_C(0)||
       evidence.size!=(uint32_t)sizeof(evidence)||evidence.version!=UINT32_C(1)||
       evidence.module_instance_id==UINT64_C(0)||
       evidence.proof_flags!=SPX_REQUIRES_FALSE_SELECTED_ORDINAL||
       evidence.postcommit_allocations!=UINT64_C(0)||evidence.host_state_flags!=UINT64_C(0)||
       count!=UINT32_C(1)||owners[0]!=UINT32_C(0)||payloads[0]!=SPX_REQUIRES_FALSE_PAYLOAD||
  (*env)->ReleaseLongArrayElements(env,output,words,status==UINT64_C(0)?0:JNI_ABORT);
  return (jlong)status;
 }}

static jlong spx_execute_identity_max(JNIEnv *env,jobject self,jlong handle,jlongArray output){{
  (void)self;
  if(spx_in_callback)return (jlong)SPX_REENTRANT_STATUS;
  if(spx_selector!=SPX_SELECTOR_IDENTITY_MAX)return (jlong)SPX_INVALID_STATUS;
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
  uint64_t status=spx_private_android_jni_v1_execute_identity_max((uint64_t)handle,&evidence,(uint32_t)sizeof(evidence));
  if(status==UINT64_C(0)){{
    uint32_t count=0,owners[2]={{0,0}};uint64_t payloads[2]={{0,0}};
    if(spx_snapshot==NULL||spx_snapshot(&count,owners,payloads,UINT32_C(2))!=UINT32_C(0)||
       evidence.size!=(uint32_t)sizeof(evidence)||evidence.version!=UINT32_C(1)||
       evidence.module_instance_id==UINT64_C(0)||
       evidence.proof_flags!=SPX_IDENTITY_MAX_PUBLICATIONS||
       evidence.postcommit_allocations!=UINT64_C(0)||evidence.host_state_flags!=UINT64_C(0)||
       count!=UINT32_C(0)||owners[0]!=UINT32_C(0)||payloads[0]!=UINT64_C(0)||
       owners[1]!=UINT32_C(0)||payloads[1]!=UINT64_C(0)){{
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
    (void)dlclose(spx_hook_image);spx_hook_image=NULL;spx_reset=NULL;spx_snapshot=NULL;spx_selector=SPX_SELECTOR_DISCARD;
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
  {{"nativeOpen","([BI)J",(void *)spx_open}},
  {{"nativeAdoptPair","(JJ[J)J",(void *)spx_adopt}},
  {{"nativeAdoptSingle","(J[J)J",(void *)spx_adopt_single}},
  {{"nativeAdoptOwned","(J[J)J",(void *)spx_adopt_owned}},
  {{"nativeConsume","(J[J)J",(void *)spx_consume}},
  {{"nativeExecuteRequiresFalse","(J[J)J",(void *)spx_execute_requires_false}},
  {{"nativeExecuteIdentityMax","(J[J)J",(void *)spx_execute_identity_max}},
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
        descriptor_literal(discard_descriptor),
        descriptor_literal(requires_false_descriptor),
        descriptor_literal(identity_max_descriptor),
    )
}

#[cfg(test)]
mod tests {
    use super::{descriptor_literal, render_jni_shim, render_provider};

    #[test]
    fn generated_units_freeze_hooks_registration_and_known_answers() {
        let provider = render_provider("/* provider */");
        assert!(provider.contains("spx_private_android_jni_finalizer_snapshot_v1"));
        assert!(provider.contains("spx_v3_generated_finalize"));
        assert!(provider.contains("static _Thread_local uint32_t spx_jni_finalizer_count"));
        let shim = render_jni_shim(&[0x53, 0x50, 0x58], &[0x52, 0x46], &[0x49, 0x44]);
        for required in [
            "static const uint8_t spx_descriptor[]={0x53,0x50,0x58,}",
            "static const uint8_t spx_rf_descriptor[]={0x52,0x46,}",
            "static const uint8_t spx_id_descriptor[]={0x49,0x44,}",
            "dev/semaprax/runtime/NativeBridge",
            "dev/semaprax/runtime/DeclaredFixtureException",
            "nativeOpen\",\"([BI)J",
            "nativeAdoptPair\",\"(JJ[J)J",
            "nativeAdoptSingle\",\"(J[J)J",
            "nativeAdoptOwned\",\"(J[J)J",
            "nativeConsume\",\"(J[J)J",
            "nativeExecuteRequiresFalse\",\"(J[J)J",
            "nativeExecuteIdentityMax\",\"(J[J)J",
            "nativeProbeException",
            "SPX_REENTRANT_STATUS UINT64_C(0x0000002d0000000b)",
            "SPX_WRONG_THREAD_STATUS UINT64_C(0x0000002d00000002)",
            "SPX_UNEXPECTED_STATUS UINT64_C(0x0000004500000001)",
            "SPX_SELECTOR_DISCARD INT32_C(0)",
            "SPX_SELECTOR_REQUIRES_FALSE INT32_C(1)",
            "SPX_SELECTOR_IDENTITY_MAX INT32_C(2)",
            "SPX_REQUIRES_FALSE_SELECTED_ORDINAL UINT64_C(1)",
            "SPX_REQUIRES_FALSE_PAYLOAD UINT64_C(18446744073709551615)",
            "SPX_IDENTITY_MAX_PUBLICATIONS UINT64_C(2)",
            "selector!=SPX_SELECTOR_DISCARD&&selector!=SPX_SELECTOR_REQUIRES_FALSE&&selector!=SPX_SELECTOR_IDENTITY_MAX",
            "spx_selector!=SPX_SELECTOR_DISCARD)return (jlong)SPX_INVALID_STATUS",
            "spx_selector!=SPX_SELECTOR_REQUIRES_FALSE)return (jlong)SPX_INVALID_STATUS",
            "spx_selector!=SPX_SELECTOR_IDENTITY_MAX)return (jlong)SPX_INVALID_STATUS",
            "spx_selector=(int)selector",
            "spx_reset()!=UINT32_C(0)",
            "reset_info.dli_fbase!=snapshot_info.dli_fbase",
            "strcmp(canonical_path,reset_path)!=0",
            "spx_in_callback=1",
            "spx_in_callback=0",
            "evidence.size!=(uint32_t)sizeof(evidence)",
            "owners[0]!=UINT32_C(1)||payloads[0]!=UINT64_C(13)",
            "evidence.proof_flags!=SPX_REQUIRES_FALSE_SELECTED_ORDINAL",
            "count!=UINT32_C(1)||owners[0]!=UINT32_C(0)||payloads[0]!=SPX_REQUIRES_FALSE_PAYLOAD",
            "owners[1]!=UINT32_C(0)||payloads[1]!=UINT64_C(0)",
            "evidence.proof_flags!=SPX_IDENTITY_MAX_PUBLICATIONS",
            "count!=UINT32_C(0)||owners[0]!=UINT32_C(0)||payloads[0]!=UINT64_C(0)",
            "spx_private_android_jni_v1_adopt_single((uint64_t)payload,&handle)",
            "spx_private_android_jni_v1_adopt_owned((uint64_t)payload,&handle)",
            "spx_private_android_jni_v1_execute_requires_false((uint64_t)handle,&evidence,(uint32_t)sizeof(evidence))",
            "spx_private_android_jni_v1_execute_identity_max((uint64_t)handle,&evidence,(uint32_t)sizeof(evidence))",
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

    #[test]
    fn descriptor_literals_are_exact_byte_hex() {
        assert_eq!(descriptor_literal(&[0x00, 0xff]), "0x00,0xff,");
        assert_eq!(descriptor_literal(&[]), "");
    }
}
