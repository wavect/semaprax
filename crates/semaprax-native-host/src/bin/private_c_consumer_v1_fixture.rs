//! Generate the private plain-C callable-v3 dynamic consumer fixture lane.
//!
//! Writes one directory containing, for the three canonical corpus directions
//! (discard-two success, requires-false semantic failure, identity-max owned
//! publication): one generated provider translation unit with a finalizer
//! marker, its exact descriptor bytes, one manifest, and one hand-written
//! strict-C consumer that dlopens the provider and drives the descriptor
//! getter + execute + settle wire sequence directly.

use std::error::Error;
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use semaprax::codegen::{
    emit_private_native_callable_v3_corpus_fixture, emit_private_native_callable_v3_fixture,
    PrivateNativeCallableV3Artifact, PrivateNativeCallableV3Fixture,
};
use semaprax::conformance::{TraceEventKind, TraceOutcome, TraceResult};
use semaprax::hir::DeclarationId;
use semaprax::owned_resource_corpus::{
    build_owned_resource_corpus_v1, OwnedResourceCorpus, OwnedResourceCorpusArgument,
};
use semaprax::semantic_trace::build_semantic_event_dictionary;

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    let output = PathBuf::from(arguments.next().ok_or_else(usage)?);
    if arguments.next().is_some() || !output.is_absolute() {
        return Err(usage().into());
    }
    fs::create_dir(&output).map_err(|error| io::Error::other(format!("create lane: {error}")))?;
    let output = fs::canonicalize(output)
        .map_err(|error| io::Error::other(format!("canonicalize lane: {error}")))?;

    let corpus = build_owned_resource_corpus_v1()
        .map_err(|error| io::Error::other(format!("build owned corpus: {error}")))?;
    let identity_case = corpus
        .cases
        .iter()
        .find(|case| case.scenario_id == "identity-max")
        .ok_or_else(|| io::Error::other("identity-max corpus case is absent"))?;
    if identity_case.expected_owned_result_ordinal != Some(0) {
        return Err(io::Error::other("identity-max result ordinal diverged").into());
    }
    let discard = discard_case(&corpus)?;
    let requires_false = corpus_case(&corpus, "requires-false")?;
    let identity_max = corpus_case(&corpus, "identity-max")?;

    write_new(&output.join("consumer.c"), CONSUMER_C.as_bytes())?;
    for case in [discard, requires_false, identity_max] {
        emit_case(&output, case)?;
    }
    Ok(())
}

fn usage() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "expected exactly one absolute output directory path",
    )
}

struct CaseSpec {
    scenario: &'static str,
    artifact: PrivateNativeCallableV3Artifact,
    arguments: Vec<Argument>,
    decision: (u32, u32),
    outcome: (u32, u32, u64),
    candidate: (u32, u32),
    published: Option<u32>,
    finalizers: Vec<(u32, u64)>,
}

#[derive(Clone, Copy)]
enum Argument {
    Owned(u64),
    Bool(bool),
}

impl Argument {
    fn kind(self) -> &'static str {
        match self {
            Self::Owned(_) => "owned",
            Self::Bool(_) => "bool",
        }
    }

    fn value(self) -> u64 {
        match self {
            Self::Owned(value) => value,
            Self::Bool(value) => u64::from(value),
        }
    }
}

fn discard_case(corpus: &OwnedResourceCorpus) -> Result<CaseSpec, Box<dyn Error>> {
    let artifact = emit_private_native_callable_v3_fixture(
        &corpus.program,
        &DeclarationId::new("token.discard-two"),
        PrivateNativeCallableV3Fixture::ScalarDiscardTwo,
    )
    .map_err(|error| io::Error::other(format!("emit discard-two provider: {error}")))?;
    // Payloads are host-chosen exactly as in the sealed iOS Simulator and
    // Android lanes; owner 1 finalizes first with 13, then owner 0 with 11.
    let arguments = vec![Argument::Owned(11), Argument::Owned(13)];
    // These ordinals seal the ScalarDiscardTwo witness trace.
    let ordinals = vec![1, 2, 3, 4, 5];
    let descriptor = read_descriptor(artifact.descriptor())?;
    let order = execute_finalizers(&descriptor.graph, &ordinals, 1, 0)?;
    if order != [1, 0] {
        return Err(
            io::Error::other(format!("discard-two finalizer walk diverged: {order:?}")).into(),
        );
    }
    validate_direction(&descriptor, &arguments, (1, 0, 0), None)?;
    let finalizers = order.into_iter().zip([13_u64, 11_u64]).collect::<Vec<_>>();
    Ok(CaseSpec {
        scenario: "discard-two",
        artifact,
        arguments,
        decision: (1, 0),
        outcome: (1, 0, 0),
        candidate: (1, 0),
        published: None,
        finalizers,
    })
}

fn corpus_case(
    corpus: &OwnedResourceCorpus,
    scenario: &'static str,
) -> Result<CaseSpec, Box<dyn Error>> {
    let case = corpus
        .cases
        .iter()
        .find(|case| case.scenario_id == scenario)
        .ok_or_else(|| io::Error::other(format!("{scenario} corpus case is absent")))?;
    let function_id = DeclarationId::new(case.function_id);
    let dictionary = build_semantic_event_dictionary(&corpus.program, &function_id)
        .map_err(|error| io::Error::other(format!("{scenario} semantic dictionary: {error}")))?;
    let ordinals = case
        .reference
        .events
        .iter()
        .map(|event| {
            dictionary
                .ordinal_for(&event.event)
                .ok_or_else(|| io::Error::other(format!("{scenario} event is undictionaried")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let artifact = emit_private_native_callable_v3_corpus_fixture(
        &corpus.program,
        &function_id,
        &case.arguments,
        case.expected_owned_result_ordinal,
        &case.reference,
    )
    .map_err(|error| io::Error::other(format!("emit {scenario} provider: {error}")))?;
    let descriptor = read_descriptor(artifact.descriptor())?;
    let arguments = case
        .arguments
        .iter()
        .map(|argument| match *argument {
            OwnedResourceCorpusArgument::Owned(payload) => Ok(Argument::Owned(payload)),
            OwnedResourceCorpusArgument::Bool(value) => Ok(Argument::Bool(value)),
            OwnedResourceCorpusArgument::I64(_) => Err(io::Error::other(format!(
                "{scenario} i64 shape is outside the C consumer lane"
            ))),
        })
        .collect::<Result<Vec<_>, _>>()?;
    match &case.reference.outcome {
        TraceOutcome::Success {
            result: TraceResult::Owned { .. },
        } => {
            let owner = u32::try_from(case.expected_owned_result_ordinal.ok_or_else(|| {
                io::Error::other(format!("{scenario} owned outcome lacks its ordinal"))
            })?)
            .map_err(|_| io::Error::other(format!("{scenario} ordinal exceeds u32")))?;
            let payload = owned_payloads(&arguments)
                .nth(owner as usize)
                .copied()
                .ok_or_else(|| io::Error::other(format!("{scenario} payload is absent")))?;
            let order = execute_finalizers(&descriptor.graph, &ordinals, 2, 0)?;
            let direction = (3_u32, owner, payload);
            validate_direction(&descriptor, &arguments, direction, Some(owner))?;
            let finalizers = order.into_iter().zip(payload_values(&arguments)).collect();
            Ok(CaseSpec {
                scenario,
                artifact,
                arguments,
                decision: (3, owner),
                outcome: direction,
                candidate: (3, owner),
                published: Some(owner),
                finalizers,
            })
        }
        TraceOutcome::Success { .. } => {
            Err(io::Error::other(format!("{scenario} scalar success is outside this lane")).into())
        }
        TraceOutcome::Failure { .. } => {
            let selected_ordinal = case
                .reference
                .events
                .iter()
                .find_map(|event| {
                    matches!(event.event, TraceEventKind::SelectFailure { .. })
                        .then(|| dictionary.ordinal_for(&event.event))
                        .flatten()
                })
                .ok_or_else(|| io::Error::other(format!("{scenario} trace has no selection")))?;
            let order = execute_finalizers(&descriptor.graph, &ordinals, 3, selected_ordinal)?;
            let direction = (2_u32, selected_ordinal, 0_u64);
            validate_direction(&descriptor, &arguments, direction, None)?;
            let finalizers = order.into_iter().zip(payload_values(&arguments)).collect();
            Ok(CaseSpec {
                scenario,
                artifact,
                arguments,
                decision: (2, 0),
                outcome: direction,
                candidate: (2, 0),
                published: None,
                finalizers,
            })
        }
    }
}

fn payload_values(arguments: &[Argument]) -> Vec<u64> {
    arguments
        .iter()
        .filter_map(|argument| match argument {
            Argument::Owned(payload) => Some(*payload),
            Argument::Bool(_) => None,
        })
        .collect()
}

fn owned_payloads(arguments: &[Argument]) -> impl Iterator<Item = &u64> {
    arguments.iter().filter_map(|argument| match argument {
        Argument::Owned(payload) => Some(payload),
        Argument::Bool(_) => None,
    })
}

fn emit_case(output: &Path, case: CaseSpec) -> Result<(), Box<dyn Error>> {
    let scenario = case.scenario;
    let marker_path = output.join(format!("finalizers-{scenario}.marker"));
    let provider_path = output.join(format!("provider-{scenario}.c"));
    let descriptor_path = output.join(format!("descriptor-{scenario}.bin"));
    let library = library_name(scenario);
    let translation_unit = format!(
        r#"{provider}
#include <stdio.h>
static void spx_v3_generated_finalize(uint32_t owner,uint64_t payload){{
  FILE *file=fopen({marker},"ab");
  if(file!=NULL){{(void)fprintf(file,"%u:%llu\n",(unsigned)owner,(unsigned long long)payload);(void)fclose(file);}}
}}
"#,
        provider = case.artifact.source(),
        marker = c_string_literal(&marker_path),
    );
    write_new(&provider_path, translation_unit.as_bytes())?;
    write_new(&descriptor_path, case.artifact.descriptor())?;
    let descriptor = read_descriptor(case.artifact.descriptor())?;
    let finalizers = if case.finalizers.is_empty() {
        "none".to_owned()
    } else {
        case.finalizers
            .iter()
            .map(|(owner, payload)| format!("{owner}:{payload}"))
            .collect::<Vec<_>>()
            .join(",")
    };
    let mut manifest = String::new();
    writeln!(manifest, "version 1")?;
    writeln!(manifest, "case {scenario}")?;
    writeln!(manifest, "library {library}")?;
    writeln!(
        manifest,
        "symbols {} {} {}",
        case.artifact.getter_symbol(),
        case.artifact.execute_symbol(),
        case.artifact.settle_symbol()
    )?;
    for name in [descriptor_path.file_name(), marker_path.file_name()] {
        let name = name.ok_or_else(|| io::Error::other("fixture file name is absent"))?;
        writeln!(
            manifest,
            "{} {}",
            label_for(name),
            name.to_str().expect("UTF-8")
        )?;
    }
    writeln!(
        manifest,
        "wires {} {} {} {} {} {}",
        descriptor.request,
        descriptor.frame,
        descriptor.response,
        descriptor.decision,
        descriptor.candidate,
        descriptor.resource_count
    )?;
    for argument in &case.arguments {
        writeln!(
            manifest,
            "argument {} {}",
            argument.kind(),
            argument.value()
        )?;
    }
    writeln!(manifest, "decision {} {}", case.decision.0, case.decision.1)?;
    writeln!(
        manifest,
        "outcome {} {} {}",
        case.outcome.0, case.outcome.1, case.outcome.2
    )?;
    writeln!(
        manifest,
        "candidate {} {}",
        case.candidate.0, case.candidate.1
    )?;
    match case.published {
        None => writeln!(manifest, "published none")?,
        Some(owner) => writeln!(manifest, "published {owner}")?,
    }
    writeln!(manifest, "finalizers {finalizers}")?;
    write_new(
        &output.join(format!("manifest-{scenario}.txt")),
        manifest.as_bytes(),
    )?;
    Ok(())
}

fn label_for(name: &std::ffi::OsStr) -> &'static str {
    let name = name.to_str().expect("fixture file name is UTF-8");
    if name.starts_with("descriptor-") {
        "descriptor"
    } else {
        "marker"
    }
}

fn library_name(scenario: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("libsemaprax-c-consumer-{scenario}.dylib")
    } else {
        format!("libsemaprax-c-consumer-{scenario}.so")
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.flush()
}

fn c_string_literal(path: &Path) -> String {
    format!(
        "\"{}\"",
        path.to_str()
            .expect("fixture path is UTF-8")
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    )
}

#[derive(Clone, Copy)]
enum ParameterKind {
    I64,
    Bool,
    Owned(u32),
}

impl ParameterKind {
    fn wire_bytes(self) -> u32 {
        match self {
            Self::I64 => 16,
            Self::Bool => 12,
            Self::Owned(_) => 20,
        }
    }
}

struct DescriptorView {
    request: u32,
    response: u32,
    frame: u32,
    decision: u32,
    action_evidence: u32,
    candidate: u32,
    resource_count: u32,
    parameters: Vec<ParameterKind>,
    result_owned_owner: Option<u32>,
    graph: GraphView,
}

struct GraphView {
    starts: Vec<u32>,
    edges: Vec<(u32, u32, GraphActionView)>,
}

enum GraphActionView {
    Finalize(u32),
    Stage,
    Certify {
        ordinals: Vec<u32>,
        outcome: u32,
        detail: u32,
    },
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    fn truncated() -> io::Error {
        io::Error::other("descriptor projection is truncated")
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], io::Error> {
        let end = self.at.checked_add(count).ok_or_else(Self::truncated)?;
        match self.bytes.get(self.at..end) {
            None => Err(Self::truncated()),
            Some(value) => {
                self.at = end;
                Ok(value)
            }
        }
    }

    fn u32(&mut self) -> Result<u32, io::Error> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().expect("four bytes")))
    }

    fn usize(&mut self) -> Result<usize, io::Error> {
        usize::try_from(self.u32()?).map_err(|_| io::Error::other("descriptor size exceeds usize"))
    }

    fn text(&mut self) -> Result<String, io::Error> {
        let length = self.usize()?;
        let bytes = self.take(length)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| io::Error::other("descriptor text is UTF-8"))
    }

    fn ordinals(&mut self) -> Result<Vec<u32>, io::Error> {
        let count = self.usize()?;
        (0..count)
            .map(|_| self.u32())
            .collect::<Result<Vec<_>, _>>()
    }
}

fn read_descriptor(bytes: &[u8]) -> Result<DescriptorView, io::Error> {
    let mut reader = Reader::new(bytes);
    if reader.take(8)? != b"SPXNABI3" || reader.u32()? != 3 || reader.u32()? != 20 {
        return Err(io::Error::other(
            "descriptor header is not canonical SPXNABI3",
        ));
    }
    let total = reader.usize()?;
    if total != bytes.len() {
        return Err(io::Error::other("descriptor total length diverges"));
    }
    let _target = reader.text()?;
    let _linkage = reader.u32()?;
    for _ in 0..19 {
        if reader.take(32)?.iter().all(|byte| *byte == 0) {
            return Err(io::Error::other("descriptor fingerprint is uninitialized"));
        }
    }
    let _module = reader.text()?;
    let _function = reader.text()?;
    let _getter = reader.text()?;
    let _execute = reader.text()?;
    let _settle = reader.text()?;
    if reader.u32()? != 3 || reader.u32()? != 0x03ff {
        return Err(io::Error::other("descriptor ABI obligations diverge"));
    }
    let mut capacities = [0_u32; 15];
    for capacity in &mut capacities {
        *capacity = reader.u32()?;
    }
    let resource_count = capacities[9];
    let parameter_count = reader.usize()?;
    let mut parameters = Vec::with_capacity(parameter_count);
    for expected_index in 0..parameter_count {
        let tag = reader.u32()?;
        if reader.usize()? != expected_index {
            return Err(io::Error::other("descriptor parameter index diverges"));
        }
        let _value = reader.text()?;
        match tag {
            1 => match reader.u32()? {
                1 => parameters.push(ParameterKind::I64),
                2 => parameters.push(ParameterKind::Bool),
                other => {
                    return Err(io::Error::other(format!(
                        "descriptor scalar kind {other} is unsupported"
                    )))
                }
            },
            2 => {
                let owner_ordinal = reader.u32()?;
                let _resource = reader.text()?;
                let _lifecycle = reader.text()?;
                if reader.u32()? != 1 {
                    return Err(io::Error::other("descriptor payload wire kind diverges"));
                }
                parameters.push(ParameterKind::Owned(owner_ordinal));
            }
            other => {
                return Err(io::Error::other(format!(
                    "descriptor parameter kind {other} is unsupported"
                )))
            }
        }
    }
    let result_owned_owner = match reader.u32()? {
        1 => None,
        2 => {
            let index = reader.usize()?;
            let _value = reader.text()?;
            let owner_ordinal = reader.u32()?;
            if !matches!(
                parameters.get(index),
                Some(ParameterKind::Owned(admitted)) if *admitted == owner_ordinal
            ) {
                return Err(io::Error::other("descriptor owned result diverges"));
            }
            Some(owner_ordinal)
        }
        other => {
            return Err(io::Error::other(format!(
                "descriptor result kind {other} is unsupported"
            )))
        }
    };
    let graph_length = reader.usize()?;
    let graph_bytes = reader.take(graph_length)?;
    if reader.at != bytes.len() {
        return Err(io::Error::other("descriptor has trailing bytes"));
    }
    let graph = read_graph(graph_bytes, resource_count)?;
    Ok(DescriptorView {
        request: capacities[0],
        response: capacities[1],
        frame: capacities[2],
        decision: capacities[3],
        action_evidence: capacities[4],
        candidate: capacities[5],
        resource_count,
        parameters,
        result_owned_owner,
        graph,
    })
}

fn read_graph(bytes: &[u8], resource_count: u32) -> Result<GraphView, io::Error> {
    let mut reader = Reader::new(bytes);
    if reader.u32()? != 3 {
        return Err(io::Error::other("settlement graph version diverges"));
    }
    let _function = reader.text()?;
    reader.take(96)?;
    if reader.usize()? != resource_count as usize {
        return Err(io::Error::other("settlement graph resource count diverges"));
    }
    let checkpoint_count = reader.usize()?;
    for expected_id in 1..=checkpoint_count {
        if reader.u32()? as usize != expected_id {
            return Err(io::Error::other("settlement checkpoints are not dense"));
        }
        let width = reader.usize()?;
        if width != resource_count as usize {
            return Err(io::Error::other("settlement checkpoint width diverges"));
        }
        reader.take(4 * width)?;
        match reader.u32()? {
            0..=2 => {}
            3 => {
                reader.u32()?;
            }
            other => {
                return Err(io::Error::other(format!(
                    "settlement checkpoint outcome {other} is invalid"
                )))
            }
        }
        reader.ordinals()?;
        reader.ordinals()?;
    }
    let starts = reader.ordinals()?;
    let edge_count = reader.usize()?;
    let mut edges = Vec::new();
    for _ in 0..edge_count {
        let from = reader.u32()?;
        let to = reader.u32()?;
        let action = match reader.u32()? {
            1 => GraphActionView::Finalize(reader.u32()?),
            2 => {
                let staged_owner = reader.u32()?;
                if staged_owner >= resource_count {
                    return Err(io::Error::other(
                        "settlement graph stage owner exceeds the resources",
                    ));
                }
                GraphActionView::Stage
            }
            3 => {
                reader.take(32)?;
                let ordinals = reader.ordinals()?;
                let outcome = reader.u32()?;
                let detail = if outcome == 3 { reader.u32()? } else { 0 };
                GraphActionView::Certify {
                    ordinals,
                    outcome,
                    detail,
                }
            }
            other => {
                return Err(io::Error::other(format!(
                    "settlement graph action {other} is invalid"
                )))
            }
        };
        edges.push((from, to, action));
    }
    if reader.at != bytes.len() {
        return Err(io::Error::other("settlement graph has trailing bytes"));
    }
    Ok(GraphView { starts, edges })
}

fn execute_finalizers(
    graph: &GraphView,
    ordinals: &[u32],
    outcome_tag: u32,
    outcome_detail: u32,
) -> Result<Vec<u32>, io::Error> {
    fn walk(
        graph: &GraphView,
        checkpoint: u32,
        ordinals: &[u32],
        outcome_tag: u32,
        outcome_detail: u32,
        path: &mut Vec<u32>,
        witnesses: &mut Vec<Vec<u32>>,
    ) {
        for (from, to, action) in &graph.edges {
            if *from != checkpoint {
                continue;
            }
            match action {
                GraphActionView::Finalize(owner) => {
                    path.push(*owner);
                    walk(
                        graph,
                        *to,
                        ordinals,
                        outcome_tag,
                        outcome_detail,
                        path,
                        witnesses,
                    );
                    path.pop();
                }
                GraphActionView::Stage => {
                    walk(
                        graph,
                        *to,
                        ordinals,
                        outcome_tag,
                        outcome_detail,
                        path,
                        witnesses,
                    );
                }
                GraphActionView::Certify {
                    ordinals: evidence_ordinals,
                    outcome,
                    detail,
                } => {
                    if evidence_ordinals.as_slice() == ordinals
                        && *outcome == outcome_tag
                        && *detail == outcome_detail
                    {
                        witnesses.push(path.clone());
                    }
                }
            }
        }
    }

    if graph.starts.len() != 1 {
        return Err(io::Error::other("settlement graph start is not unique"));
    }
    let mut witnesses = Vec::new();
    let mut path = Vec::new();
    walk(
        graph,
        graph.starts[0],
        ordinals,
        outcome_tag,
        outcome_detail,
        &mut path,
        &mut witnesses,
    );
    if witnesses.len() != 1 {
        return Err(io::Error::other("graph witness path is not unique"));
    }
    Ok(witnesses.pop().expect("one witness path"))
}

fn validate_direction(
    descriptor: &DescriptorView,
    arguments: &[Argument],
    outcome: (u32, u32, u64),
    published: Option<u32>,
) -> Result<(), io::Error> {
    if descriptor.decision != 172 || descriptor.action_evidence != 196 {
        return Err(io::Error::other("descriptor fixed wire sizes diverged"));
    }
    let owned_count = owned_payloads(arguments).count();
    if owned_count != descriptor.resource_count as usize {
        return Err(io::Error::other(
            "argument ownership diverges from descriptor",
        ));
    }
    let mut next_owner = 0_u32;
    for parameter in &descriptor.parameters {
        if let ParameterKind::Owned(owner_ordinal) = *parameter {
            if owner_ordinal != next_owner {
                return Err(io::Error::other("descriptor owner ordinals are not dense"));
            }
            next_owner += 1;
        }
    }
    if next_owner != descriptor.resource_count {
        return Err(io::Error::other("descriptor resources exceed parameters"));
    }
    let expected_request: u32 = 104
        + descriptor
            .parameters
            .iter()
            .map(|parameter| parameter.wire_bytes())
            .sum::<u32>();
    if descriptor.request != expected_request {
        return Err(io::Error::other("descriptor request capacity diverges"));
    }
    match (descriptor.result_owned_owner, outcome.0) {
        (None, 1 | 2) => {}
        (Some(owner), 3) if owner == outcome.1 => {}
        _ => {
            return Err(io::Error::other(
                "outcome direction diverges from descriptor result",
            ))
        }
    }
    match (published, outcome.0) {
        (None, 1 | 2) => {}
        (Some(owner), 3) if owner == outcome.1 => {}
        _ => return Err(io::Error::other("publication diverges from outcome")),
    }
    Ok(())
}

const CONSUMER_C: &str = r#"#define _POSIX_C_SOURCE 200809L
#include <dlfcn.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define SPX_C_HEADER_BYTES UINT32_C(20)
#define SPX_C_DECISION_BYTES UINT32_C(172)
#define SPX_C_REQUEST_MAX UINT32_C(4096)
#define SPX_C_FRAME_MAX UINT32_C(8192)
#define SPX_C_RESPONSE_MAX UINT32_C(8192)
#define SPX_C_CANDIDATE_MAX UINT32_C(16384)
#define SPX_C_DESCRIPTOR_MAX UINT32_C(65536)
#define SPX_C_MARKER_MAX UINT32_C(4096)
#define SPX_C_RESOURCES_MAX UINT32_C(64)
#define SPX_C_ARGUMENTS_MAX UINT32_C(16)
#define SPX_C_EVENTS_MAX UINT32_C(256)

_Static_assert(sizeof(uint32_t)==4,"callable-v3 wires require exact uint32_t");
_Static_assert(sizeof(uint64_t)==8,"callable-v3 wires require exact uint64_t");
_Static_assert(SPX_C_HEADER_BYTES==20,"callable-v3 envelope header is fixed");
_Static_assert(SPX_C_DECISION_BYTES==172,"callable-v3 decision wire is exactly 172 bytes");

static const char spx_c_request_domain[]="semaprax.native-callable-request-digest.v3";
static const char spx_c_frame_domain[]="semaprax.native-callable-pre-candidate-frame-digest.v3";

struct spx_c_sha {uint32_t h[8];uint64_t bits;uint8_t block[64];uint32_t used;};
static uint32_t spx_c_rotr(uint32_t x,uint32_t n){return (x>>n)|(x<<(32-n));}
static uint32_t spx_c_be32(const uint8_t *p){return ((uint32_t)p[0]<<24)|((uint32_t)p[1]<<16)|((uint32_t)p[2]<<8)|(uint32_t)p[3];}
static void spx_c_sha_block(struct spx_c_sha *s,const uint8_t *p){
 static const uint32_t k[64]={0x428a2f98U,0x71374491U,0xb5c0fbcfU,0xe9b5dba5U,0x3956c25bU,0x59f111f1U,0x923f82a4U,0xab1c5ed5U,0xd807aa98U,0x12835b01U,0x243185beU,0x550c7dc3U,0x72be5d74U,0x80deb1feU,0x9bdc06a7U,0xc19bf174U,0xe49b69c1U,0xefbe4786U,0x0fc19dc6U,0x240ca1ccU,0x2de92c6fU,0x4a7484aaU,0x5cb0a9dcU,0x76f988daU,0x983e5152U,0xa831c66dU,0xb00327c8U,0xbf597fc7U,0xc6e00bf3U,0xd5a79147U,0x06ca6351U,0x14292967U,0x27b70a85U,0x2e1b2138U,0x4d2c6dfcU,0x53380d13U,0x650a7354U,0x766a0abbU,0x81c2c92eU,0x92722c85U,0xa2bfe8a1U,0xa81a664bU,0xc24b8b70U,0xc76c51a3U,0xd192e819U,0xd6990624U,0xf40e3585U,0x106aa070U,0x19a4c116U,0x1e376c08U,0x2748774cU,0x34b0bcb5U,0x391c0cb3U,0x4ed8aa4aU,0x5b9cca4fU,0x682e6ff3U,0x748f82eeU,0x78a5636fU,0x84c87814U,0x8cc70208U,0x90befffaU,0xa4506cebU,0xbef9a3f7U,0xc67178f2U};
 uint32_t w[64],a,b,c,d,e,f,g,h;for(uint32_t i=0;i<16;i++)w[i]=spx_c_be32(p+4*i);
 for(uint32_t i=16;i<64;i++){uint32_t x=w[i-15],y=w[i-2];w[i]=(spx_c_rotr(y,17)^spx_c_rotr(y,19)^(y>>10))+w[i-7]+(spx_c_rotr(x,7)^spx_c_rotr(x,18)^(x>>3))+w[i-16];}
 a=s->h[0];b=s->h[1];c=s->h[2];d=s->h[3];e=s->h[4];f=s->h[5];g=s->h[6];h=s->h[7];
 for(uint32_t i=0;i<64;i++){uint32_t t1=h+(spx_c_rotr(e,6)^spx_c_rotr(e,11)^spx_c_rotr(e,25))+((e&f)^((~e)&g))+k[i]+w[i];uint32_t t2=(spx_c_rotr(a,2)^spx_c_rotr(a,13)^spx_c_rotr(a,22))+((a&b)^(a&c)^(b&c));h=g;g=f;f=e;e=d+t1;d=c;c=b;b=a;a=t1+t2;}
 s->h[0]+=a;s->h[1]+=b;s->h[2]+=c;s->h[3]+=d;s->h[4]+=e;s->h[5]+=f;s->h[6]+=g;s->h[7]+=h;}
static void spx_c_sha_init(struct spx_c_sha *s){static const uint32_t h[8]={0x6a09e667U,0xbb67ae85U,0x3c6ef372U,0xa54ff53aU,0x510e527fU,0x9b05688cU,0x1f83d9abU,0x5be0cd19U};memcpy(s->h,h,sizeof(h));s->bits=0;s->used=0;}
static void spx_c_sha_update(struct spx_c_sha *s,const uint8_t *p,uint32_t n){s->bits+=(uint64_t)n*UINT64_C(8);while(n){uint32_t take=UINT32_C(64)-s->used;if(take>n)take=n;memcpy(s->block+s->used,p,take);s->used+=take;p+=take;n-=take;if(s->used==UINT32_C(64)){spx_c_sha_block(s,s->block);s->used=0;}}}
static void spx_c_sha_final(struct spx_c_sha *s,uint8_t out[32]){uint64_t bits=s->bits;s->block[s->used++]=UINT8_C(0x80);if(s->used>56){memset(s->block+s->used,0,64-s->used);spx_c_sha_block(s,s->block);s->used=0;}memset(s->block+s->used,0,56-s->used);for(uint32_t i=0;i<8;i++)s->block[56+i]=(uint8_t)(bits>>(56-8*i));spx_c_sha_block(s,s->block);for(uint32_t i=0;i<8;i++){out[4*i]=(uint8_t)(s->h[i]>>24);out[4*i+1]=(uint8_t)(s->h[i]>>16);out[4*i+2]=(uint8_t)(s->h[i]>>8);out[4*i+3]=(uint8_t)s->h[i];}}
static void spx_c_hash_field(struct spx_c_sha *s,const uint8_t *p,uint32_t n){uint8_t len[8];uint64_t total=n;for(uint32_t i=0;i<8;i++)len[i]=(uint8_t)(total>>(56-8*i));spx_c_sha_update(s,len,8);spx_c_sha_update(s,p,n);}
static uint32_t spx_c_domain_length(const char *domain){return (uint32_t)(strlen(domain)+UINT32_C(1));}
static void spx_c_framed_digest(const char *domain,const uint8_t *p,uint32_t n,uint8_t out[32]){struct spx_c_sha s;spx_c_sha_init(&s);spx_c_sha_update(&s,(const uint8_t *)domain,spx_c_domain_length(domain));spx_c_hash_field(&s,p,n);spx_c_sha_final(&s,out);}

static uint32_t spx_c_get_u32(const uint8_t *p){return (uint32_t)p[0]|((uint32_t)p[1]<<8)|((uint32_t)p[2]<<16)|((uint32_t)p[3]<<24);}
static uint64_t spx_c_get_u64(const uint8_t *p){uint64_t v=0;for(uint32_t i=0;i<8;i++)v|=(uint64_t)p[i]<<(8*i);return v;}
static void spx_c_put_u32(uint8_t *p,uint32_t v){p[0]=(uint8_t)v;p[1]=(uint8_t)(v>>8);p[2]=(uint8_t)(v>>16);p[3]=(uint8_t)(v>>24);}
static void spx_c_put_u64(uint8_t *p,uint64_t v){for(uint32_t i=0;i<8;i++)p[i]=(uint8_t)(v>>(8*i));}

typedef const uint8_t *(*spx_c_getter_fn)(void);
typedef uint32_t (*spx_c_execute_fn)(const uint8_t *,uint32_t,uint8_t *,uint32_t,uint8_t *,uint32_t);
typedef uint32_t (*spx_c_settle_fn)(uint8_t *,uint32_t,const uint8_t *,uint32_t,uint8_t *,uint32_t);

struct spx_c_argument {int owned;uint64_t value;};
struct spx_c_config {
 char scenario[64];char library[256];char getter[1024];char execute[1024];char settle[1024];
 char descriptor_name[256];char marker_name[256];
 uint64_t request_bytes,frame_bytes,response_bytes,decision_bytes,candidate_bytes,resources;
 struct spx_c_argument arguments[SPX_C_ARGUMENTS_MAX];uint32_t argument_count;
 uint64_t decision_tag,decision_detail,outcome_tag,outcome_detail,outcome_payload;
 uint64_t candidate_tag,candidate_detail;
 int has_published;uint64_t published_owner;
 char finalizers[512];
};

#define SPX_C_FAIL(step,message) \
 do {(void)fprintf(stderr,"spx c consumer v1: step %d: %s\n",(int)(step),(message));return (int)(step);} while (0)

static void spx_c_number(const char *text,int step,uint64_t *out){
 char *end=NULL;unsigned long long value=strtoull(text,&end,10);
 if(end==text||*end!='\0'){(void)fprintf(stderr,"spx c consumer v1: step %d: malformed number %s\n",step,text);exit(step);}
 *out=(uint64_t)value;}

static int spx_c_read_file(const char *path,uint8_t *buffer,uint32_t capacity,size_t *length){
 FILE *file=fopen(path,"rb");if(file==NULL)return 1;
 size_t used=fread(buffer,1,(size_t)capacity+(size_t)1,file);
 int failed=ferror(file)||used>(size_t)capacity;
 (void)fclose(file);if(failed)return 2;*length=used;return 0;}

static int spx_c_parse_manifest(const char *path,struct spx_c_config *config){
 FILE *file=fopen(path,"rb");if(file==NULL)return 1;
 unsigned seen=0;char line[2048];
 const unsigned required=(1u<<0)|(1u<<1)|(1u<<2)|(1u<<3)|(1u<<4)|(1u<<5)|(1u<<6)|(1u<<7)|(1u<<8)|(1u<<9)|(1u<<10)|(1u<<11);
 while(fgets(line,sizeof line,file)!=NULL){
  char key[32],first[1024],second[1024],third[1024];
  size_t line_length=strlen(line);
  if(line_length==0)continue;
  if(line[line_length-1]!='\n'){(void)fclose(file);return 5;}
  if(sscanf(line,"%31s",key)!=1)continue;
  if(strcmp(key,"version")==0){if(sscanf(line,"%*s %1023s",first)!=1||strcmp(first,"1")!=0){(void)fclose(file);return 2;}seen|=1u<<0;}
  else if(strcmp(key,"case")==0){if(sscanf(line,"%*s %63s",config->scenario)!=1){(void)fclose(file);return 2;}seen|=1u<<1;}
  else if(strcmp(key,"library")==0){if(sscanf(line,"%*s %255s",config->library)!=1){(void)fclose(file);return 2;}seen|=1u<<2;}
  else if(strcmp(key,"symbols")==0){
   if(sscanf(line,"%*s %1023s %1023s %1023s",first,second,third)!=3){(void)fclose(file);return 2;}
   memcpy(config->getter,first,strlen(first)+1);
   memcpy(config->execute,second,strlen(second)+1);
   memcpy(config->settle,third,strlen(third)+1);seen|=1u<<3;}
  else if(strcmp(key,"descriptor")==0){if(sscanf(line,"%*s %255s",config->descriptor_name)!=1){(void)fclose(file);return 2;}seen|=1u<<4;}
  else if(strcmp(key,"marker")==0){if(sscanf(line,"%*s %255s",config->marker_name)!=1){(void)fclose(file);return 2;}seen|=1u<<5;}
  else if(strcmp(key,"wires")==0){char fields[6][80];
   if(sscanf(line,"%*s %79s %79s %79s %79s %79s %79s",fields[0],fields[1],fields[2],fields[3],fields[4],fields[5])!=6){(void)fclose(file);return 2;}
   spx_c_number(fields[0],30,&config->request_bytes);spx_c_number(fields[1],31,&config->frame_bytes);
   spx_c_number(fields[2],32,&config->response_bytes);spx_c_number(fields[3],33,&config->decision_bytes);
   spx_c_number(fields[4],34,&config->candidate_bytes);spx_c_number(fields[5],35,&config->resources);seen|=1u<<6;}
  else if(strcmp(key,"argument")==0){char kind[16],value[80];
   if(sscanf(line,"%*s %15s %79s",kind,value)!=2){(void)fclose(file);return 2;}
   if(config->argument_count>=SPX_C_ARGUMENTS_MAX){(void)fclose(file);return 3;}
   if(strcmp(kind,"owned")==0)config->arguments[config->argument_count].owned=1;
   else if(strcmp(kind,"bool")==0)config->arguments[config->argument_count].owned=0;
   else{(void)fclose(file);return 3;}
   spx_c_number(value,36,&config->arguments[config->argument_count].value);
   config->argument_count++;}
  else if(strcmp(key,"decision")==0){char tag[64],detail[64];
   if(sscanf(line,"%*s %63s %63s",tag,detail)!=2){(void)fclose(file);return 2;}
   spx_c_number(tag,37,&config->decision_tag);spx_c_number(detail,38,&config->decision_detail);seen|=1u<<7;}
  else if(strcmp(key,"outcome")==0){char tag[64],detail[64],payload[80];
   if(sscanf(line,"%*s %63s %63s %79s",tag,detail,payload)!=3){(void)fclose(file);return 2;}
   spx_c_number(tag,39,&config->outcome_tag);spx_c_number(detail,40,&config->outcome_detail);
   spx_c_number(payload,41,&config->outcome_payload);seen|=1u<<8;}
  else if(strcmp(key,"candidate")==0){char tag[64],detail[64];
   if(sscanf(line,"%*s %63s %63s",tag,detail)!=2){(void)fclose(file);return 2;}
   spx_c_number(tag,42,&config->candidate_tag);spx_c_number(detail,43,&config->candidate_detail);seen|=1u<<9;}
  else if(strcmp(key,"published")==0){char token[64];
   if(sscanf(line,"%*s %63s",token)!=1){(void)fclose(file);return 2;}
   if(strcmp(token,"none")==0){config->has_published=0;config->published_owner=0;}
   else{config->has_published=1;spx_c_number(token,44,&config->published_owner);}seen|=1u<<10;}
  else if(strcmp(key,"finalizers")==0){if(sscanf(line,"%*s %511s",config->finalizers)!=1){(void)fclose(file);return 2;}seen|=1u<<11;}
  else{(void)fclose(file);return 4;}}
 (void)fclose(file);
 if(seen!=required)return 6;
 if(config->argument_count==0)return 7;
 return 0;}

static int spx_c_parse_descriptor_fingerprints(const uint8_t *descriptor,size_t length,
 const uint8_t **call_contract,const uint8_t **recovery_contract,const uint8_t **settlement_graph){
 if(length<(size_t)SPX_C_HEADER_BYTES||memcmp(descriptor,"SPXNABI3",8)!=0)return 1;
 if(spx_c_get_u32(descriptor+8)!=3||spx_c_get_u32(descriptor+12)!=SPX_C_HEADER_BYTES)return 2;
 if((size_t)spx_c_get_u32(descriptor+16)!=length)return 3;
 size_t at=(size_t)SPX_C_HEADER_BYTES;
 if(at+4>length)return 4;
 uint32_t target_length=spx_c_get_u32(descriptor+at);at+=4;
 if(target_length==0||at+(size_t)target_length+4>length)return 4;
 at+=(size_t)target_length;
 if(spx_c_get_u32(descriptor+at)!=1)return 5;
 at+=4;
 if(at+(size_t)19*32>length)return 6;
 *call_contract=descriptor+at+(size_t)18*32;
 *recovery_contract=descriptor+at+(size_t)8*32;
 *settlement_graph=descriptor+at+(size_t)9*32;
 return 0;}

int main(int argc,char **argv){
 static struct spx_c_config config;
 static uint8_t descriptor[SPX_C_DESCRIPTOR_MAX];
 static uint8_t request[SPX_C_REQUEST_MAX];
 static uint8_t frame[SPX_C_FRAME_MAX];
 static uint8_t response[SPX_C_RESPONSE_MAX];
 static uint8_t candidate[SPX_C_CANDIDATE_MAX];
 static uint8_t saved_candidate[SPX_C_CANDIDATE_MAX];
 static uint8_t decision[SPX_C_DECISION_BYTES];
 static uint8_t marker_bytes[SPX_C_MARKER_MAX];
 if(argc!=3)SPX_C_FAIL(1,"expected one manifest path and one scenario");
 if(spx_c_parse_manifest(argv[1],&config)!=0)SPX_C_FAIL(2,"manifest parse failed");
 if(strcmp(config.scenario,argv[2])!=0)SPX_C_FAIL(3,"scenario does not match manifest");
 if(config.request_bytes>(uint64_t)SPX_C_REQUEST_MAX||config.frame_bytes>(uint64_t)SPX_C_FRAME_MAX||
    config.response_bytes>(uint64_t)SPX_C_RESPONSE_MAX||config.candidate_bytes>(uint64_t)SPX_C_CANDIDATE_MAX||
    config.resources>(uint64_t)SPX_C_RESOURCES_MAX||config.decision_bytes!=(uint64_t)SPX_C_DECISION_BYTES)
  SPX_C_FAIL(4,"wire capacities exceed the bounded consumer buffers");
 if(config.frame_bytes<(uint64_t)(324+32)||config.request_bytes<104||config.candidate_bytes<384)
  SPX_C_FAIL(5,"wire capacities are below the fixed wire floors");

 char directory[1024];
 (void)snprintf(directory,sizeof directory,"%s",argv[1]);
 {char *last_slash=strrchr(directory,'/');
  if(last_slash==NULL)(void)memcpy(directory,".",(size_t)2);else last_slash[0]='\0';}

 char path[1400];
 #define SPX_C_PATH(step,name) \
  do{int printed=snprintf(path,sizeof path,"%s/%s",directory,(name));\
     if(printed<=0||(size_t)printed>=sizeof path)SPX_C_FAIL((step),"fixture path overflow");}while(0)

 SPX_C_PATH(6,config.descriptor_name);
 size_t descriptor_length=0;
 if(spx_c_read_file(path,descriptor,SPX_C_DESCRIPTOR_MAX,&descriptor_length)!=0)
  SPX_C_FAIL(10,"descriptor file is unreadable or oversized");
 const uint8_t *call_contract=NULL,*recovery_contract=NULL,*settlement_graph=NULL;
 if(spx_c_parse_descriptor_fingerprints(descriptor,descriptor_length,
   &call_contract,&recovery_contract,&settlement_graph)!=0)
  SPX_C_FAIL(11,"descriptor fingerprints are unavailable");

 SPX_C_PATH(7,config.marker_name);
 FILE *truncate_marker=fopen(path,"wb");
 if(truncate_marker==NULL)SPX_C_FAIL(12,"finalizer marker cannot be truncated");
 (void)fclose(truncate_marker);

 SPX_C_PATH(8,config.library);
 void *image=dlopen(path,RTLD_NOW|RTLD_LOCAL);
 if(image==NULL)SPX_C_FAIL(20,"dlopen of the generated provider failed");
 void *raw_getter=dlsym(image,config.getter);
 void *raw_execute=dlsym(image,config.execute);
 void *raw_settle=dlsym(image,config.settle);
 if(raw_getter==NULL||raw_execute==NULL||raw_settle==NULL)
  SPX_C_FAIL(21,"provider settlement symbols are missing");
 if(raw_getter==raw_execute||raw_getter==raw_settle||raw_execute==raw_settle)
  SPX_C_FAIL(22,"provider settlement symbols are aliased");
 spx_c_getter_fn getter;spx_c_execute_fn execute;spx_c_settle_fn settle;
 memcpy(&getter,&raw_getter,sizeof(getter));
 memcpy(&execute,&raw_execute,sizeof(execute));
 memcpy(&settle,&raw_settle,sizeof(settle));

 const uint8_t *provided_descriptor=getter();
 if(provided_descriptor==NULL)SPX_C_FAIL(23,"provider descriptor getter returned null");
 if(memcmp(provided_descriptor,descriptor,descriptor_length)!=0)
  SPX_C_FAIL(24,"provider descriptor diverges from the expected descriptor bytes");

 uint64_t invocation=UINT64_C(42),frame_generation=UINT64_C(7);
 static uint8_t challenge[32];memset(challenge,0x5a,sizeof challenge);

 uint64_t packed=104;
 for(uint32_t i=0;i<config.argument_count;i++)
  packed+=config.arguments[i].owned?UINT64_C(20):UINT64_C(12);
 if(packed!=config.request_bytes)SPX_C_FAIL(30,"argument packing diverges from request capacity");

 memcpy(request,"SPXNRQ03",8);
 spx_c_put_u32(request+8,3);spx_c_put_u32(request+12,SPX_C_HEADER_BYTES);
 spx_c_put_u32(request+16,(uint32_t)config.request_bytes);
 memcpy(request+20,call_contract,32);
 spx_c_put_u64(request+52,invocation);spx_c_put_u64(request+60,frame_generation);
 memcpy(request+68,challenge,32);
 spx_c_put_u32(request+100,config.argument_count);
 {uint64_t at=104,owner=0;
  for(uint32_t i=0;i<config.argument_count;i++){
   spx_c_put_u32(request+at,config.arguments[i].owned?2:1);
   spx_c_put_u32(request+at+4,i);
   if(config.arguments[i].owned){spx_c_put_u32(request+at+8,(uint32_t)owner);
    spx_c_put_u64(request+at+12,config.arguments[i].value);owner++;at+=20;}
   else{spx_c_put_u32(request+at+8,(uint32_t)config.arguments[i].value);at+=12;}}}

 memset(frame,0,(size_t)config.frame_bytes);
 memcpy(frame,"SPXNFR03",8);
 spx_c_put_u32(frame+8,3);spx_c_put_u32(frame+12,SPX_C_HEADER_BYTES);
 spx_c_put_u32(frame+16,(uint32_t)config.frame_bytes);
 memcpy(frame+20,call_contract,32);
 memcpy(frame+52,recovery_contract,32);
 memcpy(frame+84,settlement_graph,32);
 spx_c_put_u64(frame+116,invocation);spx_c_put_u64(frame+124,frame_generation);
 memcpy(frame+132,challenge,32);
 spx_c_framed_digest(spx_c_request_domain,request,(uint32_t)config.request_bytes,frame+164);
 spx_c_put_u32(frame+260,1);
 spx_c_put_u32(frame+268,1);
 spx_c_put_u32(frame+272,1);
 spx_c_put_u32(frame+320,(uint32_t)config.resources);
 {uint64_t owner=0;
  for(uint32_t i=0;i<config.argument_count;i++){
   if(!config.arguments[i].owned)continue;
   uint64_t cell=324+12*(uint64_t)owner;
   spx_c_put_u32(frame+cell,1);
   spx_c_put_u64(frame+cell+4,config.arguments[i].value);
   owner++;}}
 spx_c_framed_digest(spx_c_frame_domain,frame,(uint32_t)(config.frame_bytes-32),
   frame+(size_t)(config.frame_bytes-32));

 memset(response,0,(size_t)config.response_bytes);
 if(execute(request,(uint32_t)config.request_bytes,frame,
   (uint32_t)config.frame_bytes,response,(uint32_t)config.response_bytes)!=UINT32_C(0))
  SPX_C_FAIL(41,"execute rejected the canonical request");

 if(memcmp(response,"SPXNEX03",8)!=0||spx_c_get_u32(response+8)!=3||
    spx_c_get_u32(response+12)!=SPX_C_HEADER_BYTES)SPX_C_FAIL(42,"response envelope is not canonical");
 uint32_t declared=spx_c_get_u32(response+16);
 if(declared<156||declared>config.response_bytes)SPX_C_FAIL(43,"response declared length is out of bounds");
 for(uint64_t i=(uint64_t)declared;i<config.response_bytes;i++)
  if(response[i]!=0)SPX_C_FAIL(44,"response trailing storage is not zero");
 if(memcmp(response+20,call_contract,32)!=0||memcmp(response+52,request+52,48)!=0)
  SPX_C_FAIL(45,"response identity diverges from the call");
 {uint8_t expected_digest[32];
  spx_c_framed_digest(spx_c_request_domain,request,(uint32_t)config.request_bytes,expected_digest);
  if(memcmp(response+100,expected_digest,32)!=0)SPX_C_FAIL(46,"response request digest mismatches");}
 if(spx_c_get_u32(response+132)==0||spx_c_get_u32(response+132)!=spx_c_get_u32(frame+268))
  SPX_C_FAIL(47,"response checkpoint diverges from the frame");
 if(spx_c_get_u32(response+136)!=config.outcome_tag||spx_c_get_u32(response+140)!=config.outcome_detail||
    spx_c_get_u64(response+144)!=config.outcome_payload)
  SPX_C_FAIL(48,"response outcome diverges from the sealed direction");
 uint32_t event_count=spx_c_get_u32(response+152);
 if(event_count<1||event_count>SPX_C_EVENTS_MAX||(uint64_t)156+4*(uint64_t)event_count>(uint64_t)declared)
  SPX_C_FAIL(49,"response event storage is out of bounds");
 if(config.outcome_tag==2){
  int selected_present=0;
  for(uint32_t i=0;i<event_count;i++)
   if(spx_c_get_u32(response+156+4*i)==config.outcome_detail)selected_present=1;
  if(!selected_present)SPX_C_FAIL(50,"selected failure status is absent from the trace");}

 memset(decision,0,sizeof decision);
 memcpy(decision,"SPXNDC03",8);
 spx_c_put_u32(decision+8,3);spx_c_put_u32(decision+12,SPX_C_HEADER_BYTES);
 spx_c_put_u32(decision+16,SPX_C_DECISION_BYTES);
 memcpy(decision+20,frame+20,144);
 spx_c_put_u32(decision+164,(uint32_t)config.decision_tag);
 spx_c_put_u32(decision+168,(uint32_t)config.decision_detail);

 if(settle(frame,(uint32_t)config.frame_bytes,decision,(uint32_t)SPX_C_DECISION_BYTES,
   candidate,(uint32_t)config.candidate_bytes)!=UINT32_C(0))
  SPX_C_FAIL(60,"settle rejected the canonical decision");
 if(memcmp(candidate,"SPXNCR03",8)!=0||spx_c_get_u32(candidate+8)!=3||
    spx_c_get_u32(candidate+12)!=SPX_C_HEADER_BYTES||
    (uint64_t)spx_c_get_u32(candidate+16)!=config.candidate_bytes)
  SPX_C_FAIL(61,"candidate envelope is not canonical");
 if(memcmp(candidate+20,frame+20,144)!=0)SPX_C_FAIL(62,"candidate identity diverges from the frame");
 if(spx_c_get_u32(candidate+356)!=config.candidate_tag||
    spx_c_get_u32(candidate+360)!=config.candidate_detail)
  SPX_C_FAIL(63,"candidate outcome diverges from the sealed direction");
 if(spx_c_get_u32(candidate+364)!=0||(uint64_t)spx_c_get_u32(candidate+368)!=config.resources)
  SPX_C_FAIL(64,"candidate counts diverge");
 if(spx_c_get_u32(frame+272)!=4)SPX_C_FAIL(65,"settled frame phase is not ProviderSettled");
 {uint64_t published_count=0;
  for(uint64_t owner=0;owner<config.resources;owner++){
   uint64_t disposition_offset=372+12*(uint64_t)owner;
   uint32_t disposition=spx_c_get_u32(candidate+disposition_offset);
   uint64_t payload=spx_c_get_u64(candidate+disposition_offset+4);
   int is_published=config.has_published&&config.published_owner==owner;
   if(disposition!=(is_published?2:1))SPX_C_FAIL(66,"candidate disposition diverges");
   if(payload!=spx_c_get_u64(frame+324+12*(uint64_t)owner+4))SPX_C_FAIL(67,"candidate payload diverges");
   if(is_published)published_count++;}
  if(published_count!=(config.has_published?1:0))SPX_C_FAIL(68,"publication count diverges");}

 memcpy(saved_candidate,candidate,(size_t)config.candidate_bytes);
 if(settle(frame,(uint32_t)config.frame_bytes,decision,(uint32_t)SPX_C_DECISION_BYTES,
   candidate,(uint32_t)config.candidate_bytes)!=UINT32_C(0))
  SPX_C_FAIL(70,"repeated settle rejected an already settled frame");
 if(memcmp(saved_candidate,candidate,(size_t)config.candidate_bytes)!=0)
  SPX_C_FAIL(71,"repeated settle produced a different candidate receipt");
 if(execute(request,(uint32_t)config.request_bytes,frame,
   (uint32_t)config.frame_bytes,response,(uint32_t)config.response_bytes)!=UINT32_C(1))
  SPX_C_FAIL(72,"stale re-execute was not rejected with status word 1");

 SPX_C_PATH(9,config.marker_name);
 size_t marker_length=0;
 if(spx_c_read_file(path,marker_bytes,SPX_C_MARKER_MAX,&marker_length)!=0)
  SPX_C_FAIL(80,"finalizer marker is unreadable");
 static char rendered[1024];
 {size_t at=0,records=0,cursor=0;
  while(cursor<marker_length){
   char record[128];size_t end=cursor;
   while(end<marker_length&&marker_bytes[end]!='\n')end++;
   if(end==cursor)break;
   if(end-cursor>=sizeof record)SPX_C_FAIL(81,"finalizer record is oversized");
   memcpy(record,marker_bytes+cursor,end-cursor);record[end-cursor]='\0';
   unsigned owner=0;unsigned long long payload=0;
   if(sscanf(record,"%u:%llu",&owner,&payload)!=2)SPX_C_FAIL(82,"finalizer record is malformed");
   int printed=snprintf(rendered+at,sizeof rendered-at,records==0?"%u:%llu":",%u:%llu",
     owner,payload);
   if(printed<=0||(size_t)printed>=sizeof rendered-at)SPX_C_FAIL(83,"rendered finalizers overflowed");
   at+=(size_t)printed;records++;cursor=end+1;}
  if(records==0)(void)memcpy(rendered,"none",(size_t)5);}
 if(strcmp(rendered,config.finalizers)!=0)
  SPX_C_FAIL(84,"finalizer marker diverges from the sealed order");

 {const char *kind=config.outcome_tag==1?"scalar":(config.outcome_tag==2?"failure":"owned");
  (void)printf("SEMAPRAX_C_CONSUMER_V1_OK case=%s outcome=%s:%llu payload=%llu publication=%s finalizers=%s\n",
   config.scenario,kind,(unsigned long long)config.outcome_detail,
   (unsigned long long)config.outcome_payload,
   config.has_published?"owned":"no-owned",rendered);}
 if(fflush(stdout)!=0)SPX_C_FAIL(90,"stdout flush failed");
 return 0;}
"#;
