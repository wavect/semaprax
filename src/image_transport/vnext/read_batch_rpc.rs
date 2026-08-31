//! Optional wire envelope over the existing immutable host read-batch engine.
//! The host fixes workers; requests provide only bounded inner frame strings.
use super::*;
use std::io::{self, Write};

const METHOD: Method = Method {
    name: "workspace/read-batch",
    operation: Operation::VNext(Action::ReadBatch),
    parameters: &[
        REVISION,
        Parameter {
            name: "batch",
            kind: ParameterKind::Object("semaprax.image-read-batch-request.v1"),
            required: true,
        },
    ],
    query: true,
    payload_schema: "semaprax.image-read-batch.v1",
};

pub(super) fn method() -> &'static Method {
    &METHOD
}

impl VNextSession {
    pub(super) fn read_batch_request(
        &mut self,
        id: &RequestId,
        params: &Map<String, Value>,
        available: &[&'static Method],
    ) -> Vec<u8> {
        let Some(workers) = self.read_batch_workers else {
            return error_response(
                id,
                &invalid("wire read batches were not selected by the host"),
            );
        };
        let image = &self.image;
        let context = read_batch::ReadContext {
            image,
            package_graph: self.package_graph.as_deref(),
            registry: &self.registry,
            policy: &self.policy,
            commit_enabled: self.commit.is_some(),
            available,
        };
        // Unlike the legacy host API's all-immediate fast path, every accepted
        // outer batch crosses both source checks, even if all inner frames are
        // empty, malformed, notifications, unavailable methods or stale images.
        let prepared = self.snapshot.with_authenticated_request(|_| {
            let frames = frames(params)?;
            let reads = frames
                .iter()
                .map(|frame| read_batch::prepare_read(frame, available, image))
                .collect::<Vec<_>>();
            let rows = read_batch::execute(&reads, workers, context)?;
            let responses = rows
                .into_iter()
                .map(|row| {
                    row.map(String::from_utf8)
                        .transpose()
                        .map_err(|_| invalid("read-batch engine returned invalid UTF-8"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            // Serialize and bound the entire outer response before the final
            // source check. No individual row is exposed on drift or overflow.
            Ok(outer_response(id, image, responses))
        });
        match prepared {
            Ok(bytes) => bytes,
            Err(errors) => error_response(id, &errors),
        }
    }
}

fn frames(params: &Map<String, Value>) -> Result<Vec<&[u8]>, Vec<Diagnostic>> {
    let batch = params
        .get("batch")
        .and_then(Value::as_object)
        .filter(|batch| batch.len() == 1 && batch.contains_key("frames"))
        .ok_or_else(|| invalid("read batch must contain only frames"))?;
    let rows = batch["frames"]
        .as_array()
        .filter(|rows| (1..=16).contains(&rows.len()))
        .ok_or_else(|| invalid("read batch requires one through sixteen frame strings"))?;
    // Validate all lengths before parsing or cloning any frame content.
    rows.iter()
        .map(|row| {
            row.as_str()
                .filter(|frame| frame.len() <= MAX_REQUEST_BYTES)
                .map(str::as_bytes)
                .ok_or_else(|| invalid("read batch frame must be a string within 64 KiB"))
        })
        .collect()
}

fn outer_response(
    id: &RequestId,
    image: &ProjectSemanticImage,
    responses: Vec<Option<String>>,
) -> Vec<u8> {
    struct Bounded(Vec<u8>);
    impl Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > MAX_RESPONSE_BYTES.saturating_sub(self.0.len()) {
                return Err(io::Error::other("read batch response exceeds byte bound"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut envelope = json!({
        "schema":VNEXT_RESULT_SCHEMA,"protocol":VNEXT_PROTOCOL_SCHEMA,
        "image_revision":image.image_digest(),
        "project_revision":image.revision().project_revision(),
        "payload":{"schema":"semaprax.image-read-batch.v1",
            "responses":responses,"source_authority":false}
    });
    envelope.sort_all_objects();
    let mut encoded = Bounded(Vec::new());
    if serde_json::to_writer(&mut encoded, &envelope).is_err() {
        return codec::bounded_error_response(
            Some(id),
            -32001,
            "response exceeds configured byte limit",
            MAX_RESPONSE_BYTES,
        );
    }
    let json = String::from_utf8(encoded.0).expect("serde_json emits UTF-8");
    codec::bounded_success_response(id, &json, MAX_RESPONSE_BYTES)
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    failure("SPX-G294", message)
}
