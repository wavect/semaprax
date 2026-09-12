# assurance_manifest/model_checking/engine.rs

- Bounds · struct · L36-L40 — pub struct Bounds
- TransitionSystem · interface · L48-L89 — pub trait TransitionSystem
- initial_states · function · L59-L59 — fn initial_states(&self) -> Vec<Self::State>;
- enabled_events · function · L65-L65 — fn enabled_events(&self, state: &Self::State) -> Vec<Self::Event>;
- apply · function · L73-L73 — fn apply(&self, state: &Self::State, event: &Self::Event) -> Option<Self::State>;
- is_terminal · function · L84-L84 — fn is_terminal(&self, state: &Self::State) -> bool;
- safety_invariant · function · L88-L88 — fn safety_invariant(&self, state: &Self::State) -> Result<(), String>;
- Step · struct · L93-L97 — pub struct Step<S, E>
- Trace · type · L102-L102 — pub type Trace<S, E> = Vec<Step<S, E>>;
- LimitHit · enum · L106-L110 — pub enum LimitHit
- SafetyOutcome · enum · L119-L146 — pub enum SafetyOutcome<S, E>
- ExploreCounters · struct · L152-L156 — pub struct ExploreCounters
- ExploreReport · struct · L159-L163 — pub struct ExploreReport<S, E>
- reconstruct_trace · function · L165-L182 — fn reconstruct_trace<S, E>(parent: &BTreeMap<S, (S, E)>, state: &S) -> Trace<S, E>
- check_safety · function · L192-L337 — pub fn check_safety<T: TransitionSystem>(
- ReachabilityOutcome · enum · L345-L360 — pub enum ReachabilityOutcome<S, E>
- ReachabilityReport · struct · L363-L367 — pub struct ReachabilityReport<S, E>
- check_reachable · function · L375-L518 — pub fn check_reachable<T: TransitionSystem>(
