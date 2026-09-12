# image_transport/vnext/read_batch_rpc.rs

- METHOD · constant · L6-L19 — const METHOD: Method = Method
- method · function · L21-L23 — pub(super) fn method() -> &'static Method
- read_batch_request · function · L26-L73 — pub(super) fn read_batch_request(
- frames · function · L76-L95 — fn frames(params: &Map<String, Value>) -> Result<Vec<&[u8]>, Vec<Diagnostic>>
- outer_response · function · L97-L134 — fn outer_response(
- Bounded · struct · L102-L102 — struct Bounded(Vec<u8>);
- write · function · L104-L110 — fn write(&mut self, bytes: &[u8]) -> io::Result<usize>
- flush · function · L111-L113 — fn flush(&mut self) -> io::Result<()>
- invalid · function · L136-L138 — fn invalid(message: &'static str) -> Vec<Diagnostic>
