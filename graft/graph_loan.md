---
covers: []
---
# graph_loan.rs

- format · function · L13-L17 — macro_rules! format
- loan_plan_json · function · L19-L27 — pub(crate) fn loan_plan_json(plan: &LoanPlan) -> String
- loan_json · function · L29-L47 — fn loan_json(loan: &Loan) -> String
- place_json · function · L49-L55 — fn place_json(place: &Place) -> String
- projection_json · function · L57-L69 — fn projection_json(projection: &PlaceProjection) -> String
- point_json · function · L71-L80 — fn point_json(point: &LoanProgramPoint) -> String
- cause_json · function · L82-L93 — fn cause_json(cause: &LoanCause) -> String
- endpoint_json · function · L95-L104 — fn endpoint_json(endpoint: &LoanEndpoint) -> String
- edge_json · function · L106-L113 — fn edge_json(edge: &LoanEdge) -> String
- loan_ids_json · function · L115-L120 — fn loan_ids_json(ids: &[LoanId]) -> String
- array_json · function · L122-L127 — fn array_json<T>(values: &[T], render: impl FnMut(&T) -> String) -> String
