//! Generate private x86_64/arm64 Android providers and an Emulator runner.
//!
//! This binary is available only with `unstable-android-emulator-harness`. It
//! writes caller-selected temporary outputs and creates no public bundle or
//! compiler admission surface. JNI/Kotlin integration is deliberately outside
//! this native-process evidence tranche.

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
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    let x86_provider_output = PathBuf::from(arguments.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected absolute x86-provider.c, arm64-provider.c, and runner.c output paths",
        )
    })?);
    let arm64_provider_output = PathBuf::from(arguments.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected absolute x86-provider.c, arm64-provider.c, and runner.c output paths",
        )
    })?);
    let runner_output = PathBuf::from(arguments.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected absolute x86-provider.c, arm64-provider.c, and runner.c output paths",
        )
    })?);
    if arguments.next().is_some()
        || !x86_provider_output.is_absolute()
        || !arm64_provider_output.is_absolute()
        || !runner_output.is_absolute()
        || x86_provider_output == arm64_provider_output
        || x86_provider_output == runner_output
        || arm64_provider_output == runner_output
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected three distinct absolute output paths",
        )
        .into());
    }

    let corpus = build_owned_resource_corpus_v1()
        .map_err(|error| io::Error::other(format!("build owned corpus: {error:?}")))?;
    let x86_artifact = emit_private_native_callable_v3_android_fixture(
        &corpus.program,
        &DeclarationId::new("token.discard-two"),
        PrivateNativeCallableV3Fixture::ScalarDiscardTwo,
        PrivateNativeCallableV3AndroidTarget::X86_64,
    )
    .map_err(|error| io::Error::other(format!("emit Android Emulator fixture: {error:?}")))?;
    let arm64_artifact = emit_private_native_callable_v3_android_fixture(
        &corpus.program,
        &DeclarationId::new("token.discard-two"),
        PrivateNativeCallableV3Fixture::ScalarDiscardTwo,
        PrivateNativeCallableV3AndroidTarget::Arm64,
    )
    .map_err(|error| io::Error::other(format!("emit Android arm64 fixture: {error:?}")))?;

    write_new(
        &x86_provider_output,
        render_provider_translation_unit(x86_artifact.source()).as_bytes(),
    )?;
    write_new(
        &arm64_provider_output,
        render_provider_translation_unit(arm64_artifact.source()).as_bytes(),
    )?;
    write_new(
        &runner_output,
        render_runner_translation_unit(x86_artifact.descriptor()).as_bytes(),
    )?;
    Ok(())
}

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.flush()
}

fn render_provider_translation_unit(provider: &str) -> String {
    format!(
        r#"{provider}

#include <fcntl.h>
#include <stdlib.h>
#include <unistd.h>

static void spx_v3_generated_finalize(uint32_t owner,uint64_t payload){{
  static const char first[]="1:13\n";
  static const char second[]="0:11\n";
  static const char invalid[]="invalid\n";
  const char *marker=getenv("SEMAPRAX_ANDROID_V3_MARKER");
  const char *record=invalid;
  size_t record_len=sizeof(invalid)-1;
  int fd;
  if(owner==UINT32_C(1)&&payload==UINT64_C(13)){{record=first;record_len=sizeof(first)-1;}}
  else if(owner==UINT32_C(0)&&payload==UINT64_C(11)){{record=second;record_len=sizeof(second)-1;}}
  if(marker==NULL||marker[0]=='\0')return;
  fd=open(marker,O_WRONLY|O_APPEND|O_CREAT,0600);
  if(fd<0)return;
  (void)write(fd,record,record_len);
  (void)close(fd);
}}
"#,
    )
}

fn render_runner_translation_unit(descriptor: &[u8]) -> String {
    let mut bytes = String::new();
    for byte in descriptor {
        write!(bytes, "0x{byte:02x},").expect("writing to a string cannot fail");
    }
    format!(
        r#"#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static const uint8_t spx_private_android_descriptor[]={{{bytes}}};

struct spx_private_android_emulator_evidence_v1 {{
  uint32_t size;
  uint32_t version;
  uint32_t target;
  uint32_t retained_instance;
  uint64_t module_instance_id;
  uint32_t outcome;
  uint32_t publication;
  uint32_t receipt_nonzero;
  uint32_t candidate_nonzero;
  uint32_t identity_digests_nonzero;
  uint32_t ledger_before_nonzero;
  uint32_t ledger_after_nonzero;
  uint32_t ledger_changed;
  uint32_t poisoned;
  uint32_t draining;
  uint32_t quarantined_count;
  uint64_t postcommit_allocations;
}};

_Static_assert(sizeof(struct spx_private_android_emulator_evidence_v1)==80,"private evidence size");
_Static_assert(offsetof(struct spx_private_android_emulator_evidence_v1,module_instance_id)==16,"private evidence instance offset");
_Static_assert(offsetof(struct spx_private_android_emulator_evidence_v1,outcome)==24,"private evidence outcome offset");
_Static_assert(offsetof(struct spx_private_android_emulator_evidence_v1,receipt_nonzero)==32,"private evidence receipt offset");
_Static_assert(offsetof(struct spx_private_android_emulator_evidence_v1,identity_digests_nonzero)==40,"private evidence identity offset");
_Static_assert(offsetof(struct spx_private_android_emulator_evidence_v1,ledger_changed)==52,"private evidence ledger offset");
_Static_assert(offsetof(struct spx_private_android_emulator_evidence_v1,quarantined_count)==64,"private evidence quarantine offset");
_Static_assert(offsetof(struct spx_private_android_emulator_evidence_v1,postcommit_allocations)==72,"private evidence allocation offset");

extern uint32_t spx_private_android_emulator_v3_run(
  const uint8_t *,uint32_t,const uint8_t *,uint32_t,
  struct spx_private_android_emulator_evidence_v1 *,uint32_t
);

static int spx_private_evidence_is_exact(const struct spx_private_android_emulator_evidence_v1 *e){{
  return e->size==UINT32_C(80)&&e->version==UINT32_C(1)&&e->target==UINT32_C(1)&&
    e->retained_instance==UINT32_C(1)&&e->module_instance_id!=UINT64_C(0)&&
    e->outcome==UINT32_C(1)&&e->publication==UINT32_C(1)&&
    e->receipt_nonzero==UINT32_C(1)&&e->candidate_nonzero==UINT32_C(1)&&
    e->identity_digests_nonzero==UINT32_C(1)&&e->ledger_before_nonzero==UINT32_C(1)&&
    e->ledger_after_nonzero==UINT32_C(1)&&e->ledger_changed==UINT32_C(1)&&
    e->poisoned==UINT32_C(0)&&e->draining==UINT32_C(0)&&
    e->quarantined_count==UINT32_C(0)&&e->postcommit_allocations==UINT64_C(0);
}}

int main(int argc,char **argv){{
  struct spx_private_android_emulator_evidence_v1 evidence={{0}};
  uint32_t step;
  size_t path_len;
  if(argc!=3||(strcmp(argv[2],"O0")!=0&&strcmp(argv[2],"O2")!=0))return 90;
  path_len=strlen(argv[1]);
  if(path_len==0||path_len>UINT32_MAX)return 90;
  step=spx_private_android_emulator_v3_run(
    (const uint8_t *)argv[1],(uint32_t)path_len,
    spx_private_android_descriptor,(uint32_t)sizeof(spx_private_android_descriptor),
    &evidence,UINT32_C(80)
  );
  if(step!=UINT32_C(0)){{(void)fprintf(stderr,"private Android Emulator bridge failed at step %u\n",(unsigned)step);return 91;}}
  if(!spx_private_evidence_is_exact(&evidence))return 92;
  (void)printf("SEMAPRAX_ANDROID_EMULATOR_V3_OK %s target=x86_64-android finalizers=1:13,0:11 publication=no-owned allocations=0\n",argv[2]);
  return 0;
}}
"#,
    )
}

#[cfg(test)]
mod tests {
    use super::{render_provider_translation_unit, render_runner_translation_unit};

    #[test]
    fn translation_units_bind_marker_descriptor_evidence_and_exact_label() {
        let provider = render_provider_translation_unit("/* exact provider */");
        for required in [
            "/* exact provider */",
            "SEMAPRAX_ANDROID_V3_MARKER",
            "O_WRONLY|O_APPEND|O_CREAT,0600",
            "owner==UINT32_C(1)&&payload==UINT64_C(13)",
            "owner==UINT32_C(0)&&payload==UINT64_C(11)",
        ] {
            assert!(provider.contains(required), "missing `{required}`");
        }

        let runner = render_runner_translation_unit(&[0x53, 0x50, 0x58, 0x4e]);
        for required in [
            "0x53,0x50,0x58,0x4e,",
            "spx_private_android_emulator_v3_run",
            "_Static_assert(sizeof(struct spx_private_android_emulator_evidence_v1)==80",
            "offsetof(struct spx_private_android_emulator_evidence_v1,postcommit_allocations)==72",
            "strcmp(argv[2],\"O0\")",
            "strcmp(argv[2],\"O2\")",
            "target=x86_64-android finalizers=1:13,0:11 publication=no-owned allocations=0",
        ] {
            assert!(runner.contains(required), "missing `{required}`");
        }
    }
}
