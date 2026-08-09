//! Generate the complete private arm64 iOS Simulator callable-v3 C fixture.
//!
//! This binary is available only with `unstable-ios-simulator-harness`. It
//! writes a caller-selected temporary output and never creates a public bundle
//! or compiler admission surface.

use std::error::Error;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::PathBuf;

use semaprax::codegen::{
    emit_private_native_callable_v3_ios_fixture, PrivateNativeCallableV3Fixture,
    PrivateNativeCallableV3IosTarget,
};
use semaprax::hir::DeclarationId;
use semaprax::owned_resource_corpus::build_owned_resource_corpus_v1;

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    let output = PathBuf::from(arguments.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected one absolute output path",
        )
    })?);
    if arguments.next().is_some() || !output.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected exactly one absolute output path",
        )
        .into());
    }

    let corpus = build_owned_resource_corpus_v1()
        .map_err(|error| io::Error::other(format!("build owned corpus: {error:?}")))?;
    let artifact = emit_private_native_callable_v3_ios_fixture(
        &corpus.program,
        &DeclarationId::new("token.discard-two"),
        PrivateNativeCallableV3Fixture::ScalarDiscardTwo,
        PrivateNativeCallableV3IosTarget::SimulatorArm64,
    )
    .map_err(|error| io::Error::other(format!("emit iOS Simulator fixture: {error:?}")))?;

    let complete = render_complete_translation_unit(
        artifact.source(),
        artifact.descriptor().len(),
        artifact.getter_symbol(),
        artifact.execute_symbol(),
        artifact.settle_symbol(),
    );
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)?;
    file.write_all(complete.as_bytes())?;
    file.flush()?;
    Ok(())
}

fn render_complete_translation_unit(
    provider: &str,
    descriptor_len: usize,
    getter: &str,
    execute: &str,
    settle: &str,
) -> String {
    format!(
        r#"{provider}

#include <stdio.h>
#include <stdlib.h>

typedef const uint8_t *(*spx_private_getter_fn)(void);
typedef uint32_t (*spx_private_execute_fn)(const uint8_t *,uint32_t,uint8_t *,uint32_t,uint8_t *,uint32_t);
typedef uint32_t (*spx_private_settle_fn)(uint8_t *,uint32_t,const uint8_t *,uint32_t,uint8_t *,uint32_t);

struct spx_private_ios_simulator_evidence_v1 {{
  uint32_t size;
  uint32_t version;
  uint32_t target;
  uint32_t same_instance;
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

_Static_assert(sizeof(struct spx_private_ios_simulator_evidence_v1)==80,"private evidence size");
_Static_assert(offsetof(struct spx_private_ios_simulator_evidence_v1,module_instance_id)==16,"private evidence instance offset");
_Static_assert(offsetof(struct spx_private_ios_simulator_evidence_v1,outcome)==24,"private evidence outcome offset");
_Static_assert(offsetof(struct spx_private_ios_simulator_evidence_v1,receipt_nonzero)==32,"private evidence receipt offset");
_Static_assert(offsetof(struct spx_private_ios_simulator_evidence_v1,identity_digests_nonzero)==40,"private evidence identity offset");
_Static_assert(offsetof(struct spx_private_ios_simulator_evidence_v1,ledger_changed)==52,"private evidence ledger offset");
_Static_assert(offsetof(struct spx_private_ios_simulator_evidence_v1,quarantined_count)==64,"private evidence quarantine offset");
_Static_assert(offsetof(struct spx_private_ios_simulator_evidence_v1,postcommit_allocations)==72,"private evidence allocation offset");

extern uint32_t spx_private_ios_simulator_v3_run(
  const uint8_t *,
  uint32_t,
  spx_private_getter_fn,
  spx_private_execute_fn,
  spx_private_settle_fn,
  struct spx_private_ios_simulator_evidence_v1 *,
  uint32_t
);

static uint32_t spx_private_finalizer_count=UINT32_C(0);
static uint32_t spx_private_finalizer_owner[2]={{UINT32_MAX,UINT32_MAX}};
static uint64_t spx_private_finalizer_payload[2]={{UINT64_MAX,UINT64_MAX}};
static uint32_t spx_private_finalizer_overflow=UINT32_C(0);

static void spx_v3_generated_finalize(uint32_t owner,uint64_t payload){{
  if(spx_private_finalizer_count>=UINT32_C(2)){{spx_private_finalizer_overflow=UINT32_C(1);return;}}
  spx_private_finalizer_owner[spx_private_finalizer_count]=owner;
  spx_private_finalizer_payload[spx_private_finalizer_count]=payload;
  spx_private_finalizer_count++;
}}

static int spx_private_evidence_is_exact(const struct spx_private_ios_simulator_evidence_v1 *e){{
  return e->size==UINT32_C(80)&&e->version==UINT32_C(1)&&e->target==UINT32_C(1)&&
    e->same_instance==UINT32_C(1)&&e->module_instance_id!=UINT64_C(0)&&
    e->outcome==UINT32_C(1)&&e->publication==UINT32_C(1)&&
    e->receipt_nonzero==UINT32_C(1)&&e->candidate_nonzero==UINT32_C(1)&&
    e->identity_digests_nonzero==UINT32_C(1)&&e->ledger_before_nonzero==UINT32_C(1)&&
    e->ledger_after_nonzero==UINT32_C(1)&&e->ledger_changed==UINT32_C(1)&&
    e->poisoned==UINT32_C(0)&&e->draining==UINT32_C(0)&&
    e->quarantined_count==UINT32_C(0)&&e->postcommit_allocations==UINT64_C(0);
}}

int main(int argc,char **argv){{
  struct spx_private_ios_simulator_evidence_v1 evidence={{0}};
  uint32_t step;
  if(argc!=2||(strcmp(argv[1],"O0")!=0&&strcmp(argv[1],"O2")!=0))return 90;
  step=spx_private_ios_simulator_v3_run(
    {getter}(),
    UINT32_C({descriptor_len}),
    {getter},
    {execute},
    {settle},
    &evidence,
    UINT32_C(80)
  );
  if(step!=UINT32_C(0)){{(void)fprintf(stderr,"private iOS Simulator bridge failed at step %u\n",(unsigned)step);return 91;}}
  if(!spx_private_evidence_is_exact(&evidence))return 92;
  if(spx_private_finalizer_overflow!=UINT32_C(0)||spx_private_finalizer_count!=UINT32_C(2))return 93;
  if(spx_private_finalizer_owner[0]!=UINT32_C(1)||spx_private_finalizer_payload[0]!=UINT64_C(13))return 94;
  if(spx_private_finalizer_owner[1]!=UINT32_C(0)||spx_private_finalizer_payload[1]!=UINT64_C(11))return 95;
  (void)printf("SEMAPRAX_IOS_SIM_V3_OK %s target=arm64-simulator finalizers=1:13,0:11 publication=no-owned allocations=0\n",argv[1]);
  return 0;
}}
"#,
    )
}

#[cfg(test)]
mod tests {
    use super::render_complete_translation_unit;

    #[test]
    fn complete_translation_unit_binds_exact_symbols_evidence_and_labels() {
        let source = render_complete_translation_unit(
            "/* exact provider */",
            731,
            "spx_getter_exact",
            "spx_execute_exact",
            "spx_settle_exact",
        );
        for required in [
            "/* exact provider */",
            "spx_getter_exact()",
            "UINT32_C(731)",
            "spx_execute_exact",
            "spx_settle_exact",
            "spx_private_ios_simulator_v3_run",
            "_Static_assert(sizeof(struct spx_private_ios_simulator_evidence_v1)==80",
            "offsetof(struct spx_private_ios_simulator_evidence_v1,postcommit_allocations)==72",
            "strcmp(argv[1],\"O0\")",
            "strcmp(argv[1],\"O2\")",
            "finalizers=1:13,0:11 publication=no-owned allocations=0",
        ] {
            assert!(source.contains(required), "missing `{required}`");
        }
    }
}
