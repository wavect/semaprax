//! Host job budgets, separate from the language ProcessProvider limits
//! (`MAX_TIMEOUT`/`MAX_RUNS` in the root crate are neither reused nor widened).

use crate::diag::{HarnessDiagnostic, HarnessResult};

/// Hard totals for adapter work in one manager.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostBudget {
    pub max_jobs: u64,
    pub max_total_ms: u64,
    pub max_output_bytes: u64,
}

impl Default for HostBudget {
    fn default() -> Self {
        Self {
            max_jobs: 10_000,
            max_total_ms: 3_600_000,
            max_output_bytes: 256 * 1024 * 1024,
        }
    }
}

/// Running totals. A job is charged when it starts; time and output when it ends.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BudgetLedger {
    pub jobs: u64,
    pub total_ms: u64,
    pub output_bytes: u64,
}

impl BudgetLedger {
    /// Admit one more job, or refuse with `SPX-HPC022` once any total is spent.
    pub fn start_job(&mut self, budget: &HostBudget) -> HarnessResult<()> {
        let spent = |what: &str| {
            Err(HarnessDiagnostic::new(
                "SPX-HPC022",
                format!("host {what} budget is exhausted"),
            ))
        };
        if self.jobs >= budget.max_jobs {
            return spent("job-count");
        }
        if self.total_ms >= budget.max_total_ms {
            return spent("time");
        }
        if self.output_bytes >= budget.max_output_bytes {
            return spent("output");
        }
        self.jobs += 1;
        Ok(())
    }

    pub fn finish_job(&mut self, elapsed_ms: u64, output_bytes: u64) {
        self.total_ms = self.total_ms.saturating_add(elapsed_ms);
        self.output_bytes = self.output_bytes.saturating_add(output_bytes);
    }
}
