//! Affine invocation context; moving it preserves existing Strings, not a
//! payment certificate. No clone, second scope, temporary payer or refund.
use super::*;
pub(super) struct Context {
    phase: BuilderPhase,
    modules: usize,
    forecast: usize,
    dominant: Option<(String, usize)>,
    pending: Option<String>,
    sequential: Option<SequentialRefusal>,
}
impl Context {
    pub(super) fn take() -> Self {
        Self {
            phase: BUILDER_PHASE.with(Cell::get),
            modules: PRECHARGE_MODULES.with(Cell::get),
            forecast: PRECHARGE_BYTES.with(Cell::get),
            dominant: PRECHARGE_DOMINANT.with(|v| v.replace(None)),
            pending: PRECHARGE_PENDING.with(|v| v.replace(None)),
            sequential: PRECHARGE_SEQUENTIAL.with(|v| v.replace(None)),
        }
    }
    pub(super) fn write_help(
        &self,
        maximum: usize,
        output: &mut dyn std::fmt::Write,
    ) -> std::fmt::Result {
        let precharge = self.phase == BuilderPhase::Precharge;
        let modules = self.modules;
        let forecast = self.forecast;
        let dominant = &self.dominant;
        let pending = &self.pending;
        if let (true, Some(path)) = (precharge, pending.as_ref()) {
            return write!(output,
            "the static admission pre-charge refused before the graph builder ran, while forecasting module `{path}` on its own: that one module does not fit the {maximum}-byte budget, which already carries {forecast} bytes for {modules} earlier module(s). Splitting `{path}` into several modules cannot reduce the forecast, because the pre-charge sums every reachable module; reducing its admitted source can."
        );
        }
        let sequential = &self.sequential;
        if let (true, Some(step)) = (precharge, sequential.as_ref()) {
            let SequentialRefusal {
                path,
                earlier,
                retained,
                additional,
                resolving,
            } = step;
            return if *resolving {
                write!(output,
                "the static admission pre-charge refused before the graph builder ran, in its final sequential uncached forecast: {earlier} earlier module(s) retain {retained} bytes of filtered output, and resolving `{path}` adds its complete {additional}-byte forecast, which exceeds the {maximum}-byte budget. Each complete module forecast is charged only while that module resolves; reducing the admitted source of `{path}` or the output retained by earlier modules can reduce this peak."
            )
            } else {
                write!(output,
                "the static admission pre-charge refused before the graph builder ran, in its final sequential uncached forecast: {earlier} earlier module(s) retain {retained} bytes of filtered output, and `{path}` retains {additional} more bytes, which exceeds the {maximum}-byte budget. Retained output sums every reachable module, so splitting one module into several cannot reduce it; removing a dependency or reducing total reachable source can."
            )
            };
        }
        match (precharge, dominant.as_ref()) {
        (true, Some((path, bytes))) => write!(output,
            "the static admission pre-charge refused before the graph builder ran: forecasting {modules} reachable module(s) already reaches {forecast} bytes of the {maximum}-byte budget, and `{path}` alone forecasts {bytes} bytes of retained resolver state. The pre-charge sums every reachable module, so splitting one module into several cannot reduce it; removing a dependency or reducing total reachable source can."
        ),
        (true, None) => write!(output,
            "the static admission pre-charge refused before the graph builder ran, and before any single module was fully costed against the {maximum}-byte budget."
        ),
        (false, Some((path, bytes))) => write!(output,
            "live graph construction, not the static admission pre-charge, exceeded the budget: the pre-charge forecast {forecast} bytes for {modules} module(s) and fit within {maximum}, with `{path}` the largest at {bytes} bytes. Reducing reachable source is the only lever; the forecast itself was not the refusing bound."
        ),
        (false, None) => write!(output,
            "live graph construction, not the static admission pre-charge, exceeded the {maximum}-byte budget while allocating workspace graph structure."
        ),
    }
    }
}
#[cfg(test)]
impl Context {
    pub(super) fn dominant_pointer(&self) -> Option<*const u8> {
        self.dominant.as_ref().map(|x| x.0.as_ptr())
    }
}
