//! Deterministic Graph v23/v24 serialization for validated shared-loan plans.
//!
//! This renderer preserves every plan vector exactly as supplied. Canonical
//! construction and hostile replay belong to [`crate::loan_plan`]; Graph must
//! never sort, deduplicate, infer, or repair loan evidence.

use crate::diagnostic::quote_json;
use crate::hir::{Place, PlaceProjection};
use crate::loan_plan::{
    Loan, LoanCause, LoanEdge, LoanEndpoint, LoanId, LoanPlan, LoanPointPhase, LoanProgramPoint,
};

macro_rules! format {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

pub(crate) fn loan_plan_json(plan: &LoanPlan) -> String {
    format!(
        "{{\"kind\":\"loan_plan\",\"schema\":{},\"loans\":{},\"endpoints\":{},\"edges\":{}}}",
        quote_json(plan.schema),
        array_json(&plan.loans, loan_json),
        array_json(&plan.endpoints, endpoint_json),
        array_json(&plan.edges, edge_json),
    )
}

fn loan_json(loan: &Loan) -> String {
    format!(
        "{{\"kind\":\"loan\",\"id\":{},\"site\":{},\"origin\":{},\"parent\":{},\"start\":{},\"ends\":{},\"end_edges\":{},\"cause\":{}}}",
        loan.id.0,
        quote_json(loan.site.as_str()),
        place_json(&loan.origin),
        loan.parent.map_or_else(|| "null".to_owned(), |parent| parent.0.to_string()),
        point_json(&loan.start),
        array_json(&loan.ends, point_json),
        format!(
            "[{}]",
            crate::bounded_output::budgeted_join(
                loan.end_edges.iter().map(u16::to_string),
                ","
            )
        ),
        cause_json(&loan.cause),
    )
}

fn place_json(place: &Place) -> String {
    format!(
        "{{\"kind\":\"place\",\"root\":{},\"projections\":{}}}",
        quote_json(place.root.as_str()),
        array_json(&place.projections, projection_json),
    )
}

fn projection_json(projection: &PlaceProjection) -> String {
    match projection {
        PlaceProjection::Field(field) => format!(
            "{{\"kind\":\"field\",\"field\":{}}}",
            quote_json(field.as_str())
        ),
        PlaceProjection::VariantField { case, field } => format!(
            "{{\"kind\":\"variant_field\",\"case\":{},\"field\":{}}}",
            quote_json(case.as_str()),
            quote_json(field.as_str())
        ),
    }
}

fn point_json(point: &LoanProgramPoint) -> String {
    format!(
        "{{\"expression\":{},\"phase\":{}}}",
        quote_json(point.expression.as_str()),
        quote_json(match point.phase {
            LoanPointPhase::Before => "before",
            LoanPointPhase::After => "after",
        }),
    )
}

fn cause_json(cause: &LoanCause) -> String {
    match cause {
        LoanCause::SliceView => "{\"kind\":\"slice_view\"}".to_owned(),
        LoanCause::BorrowedCall { argument } => {
            format!("{{\"kind\":\"borrowed_call\",\"argument\":{argument}}}")
        }
        LoanCause::MatchBorrow { arm } => {
            format!("{{\"kind\":\"match_borrow\",\"arm\":{arm}}}")
        }
    }
}

fn endpoint_json(endpoint: &LoanEndpoint) -> String {
    format!(
        "{{\"kind\":\"loan_endpoint\",\"point\":{},\"live_before\":{},\"starts\":{},\"kills\":{},\"live_after\":{}}}",
        point_json(&endpoint.point),
        loan_ids_json(&endpoint.live_before),
        loan_ids_json(&endpoint.starts),
        loan_ids_json(&endpoint.kills),
        loan_ids_json(&endpoint.live_after),
    )
}

fn edge_json(edge: &LoanEdge) -> String {
    format!(
        "{{\"kind\":\"loan_edge\",\"from\":{},\"to\":{},\"live\":{}}}",
        edge.from,
        edge.to,
        loan_ids_json(&edge.live),
    )
}

fn loan_ids_json(ids: &[LoanId]) -> String {
    format!(
        "[{}]",
        crate::bounded_output::budgeted_join(ids.iter().map(|id| id.0.to_string()), ",")
    )
}

fn array_json<T>(values: &[T], render: impl FnMut(&T) -> String) -> String {
    format!(
        "[{}]",
        crate::bounded_output::budgeted_join(values.iter().map(render), ",")
    )
}
