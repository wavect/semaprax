//! Bounded name resolution for the real-socket provider.
//!
//! `ToSocketAddrs` blocks in the platform resolver for as long as that
//! resolver wants, and nothing in `std` can interrupt it. Running it inline
//! therefore means the aggregate operation deadline starts only *after*
//! resolution finishes, which is exactly the hole this module closes.
//!
//! Two shapes satisfy the contract:
//!
//! - a host injects its own [`NameResolver`] that is bounded by construction
//!   (a cache, a static table, an async stub resolver);
//! - the shipped [`SystemResolver`] runs the platform resolver on a worker it
//!   *owns*. When the caller's remaining budget runs out the worker is not
//!   detached and forgotten: it stays registered on the resolver, is reaped
//!   once it finishes, and [`SystemResolver::pending`] reports how many are
//!   still outstanding.
//!
//! Retaining a worker is not the same as cancelling it. `getaddrinfo` cannot
//! be aborted from another thread, so the honest claim is that the *caller*
//! stops waiting on time and the abandoned work stays accounted for. No forced
//! cancellation is promised where the host cannot enforce it.

use std::fmt::Debug;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs as _};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::Duration;

use super::deadline::ScriptedClock;

/// Why a bounded resolution produced no addresses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolveFailure {
    /// The host is not an admitted endpoint shape.
    InvalidEndpoint,
    /// The resolver answered, but with no usable address.
    NotFound,
    /// The remaining budget ran out before the resolver answered.
    DeadlineExceeded,
}

/// Name resolution bounded by a caller-supplied budget.
///
/// `budget` is the duration *remaining* on the aggregate operation deadline,
/// never a fresh per-step timeout. An implementation must return within it.
pub trait NameResolver: Debug + Send + Sync {
    /// Resolve `host:port` within `budget`.
    fn resolve(
        &self,
        host: &str,
        port: u16,
        budget: Duration,
    ) -> Result<Vec<SocketAddr>, ResolveFailure>;

    /// Release whatever the resolver still owns when the invocation settles.
    fn settle(&self) {}
}

/// The shipped resolver over the platform name service.
#[derive(Debug, Default)]
pub struct SystemResolver {
    /// Workers whose caller already gave up waiting. They are retained, not
    /// detached, and reaped as they finish.
    abandoned: Mutex<Vec<JoinHandle<()>>>,
}

impl SystemResolver {
    /// Create a resolver owning no workers.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many abandoned workers are still running. A test asserts this
    /// returns to zero, which is what "no unmanaged resolver task" means here.
    #[must_use]
    pub fn pending(&self) -> usize {
        let mut abandoned = self.abandoned.lock().unwrap_or_else(|error| {
            self.abandoned.clear_poison();
            error.into_inner()
        });
        abandoned.retain(|worker| !worker.is_finished());
        abandoned.len()
    }

    fn retain(&self, worker: JoinHandle<()>) {
        let mut abandoned = self.abandoned.lock().unwrap_or_else(|error| {
            self.abandoned.clear_poison();
            error.into_inner()
        });
        abandoned.retain(|pending| !pending.is_finished());
        abandoned.push(worker);
    }
}

impl NameResolver for SystemResolver {
    fn resolve(
        &self,
        host: &str,
        port: u16,
        budget: Duration,
    ) -> Result<Vec<SocketAddr>, ResolveFailure> {
        // A literal address needs no name service at all, so it costs nothing
        // against the budget and spawns no worker.
        if let Ok(address) = host.parse::<IpAddr>() {
            return Ok(vec![SocketAddr::new(address, port)]);
        }
        if budget.is_zero() {
            return Err(ResolveFailure::DeadlineExceeded);
        }
        let (sender, receiver) = mpsc::channel();
        let owned_host = host.to_owned();
        let worker = std::thread::Builder::new()
            .name("semaprax-resolve".to_owned())
            .spawn(move || {
                let resolved = (owned_host.as_str(), port)
                    .to_socket_addrs()
                    .map(|addresses| addresses.collect::<Vec<_>>());
                // The receiver is gone once the caller's budget expired; that
                // is expected, not an error.
                let _ = sender.send(resolved);
            })
            .map_err(|_| ResolveFailure::DeadlineExceeded)?;
        match receiver.recv_timeout(budget) {
            Ok(Ok(addresses)) if !addresses.is_empty() => {
                let _ = worker.join();
                Ok(addresses)
            }
            Ok(_) => {
                let _ = worker.join();
                Err(ResolveFailure::NotFound)
            }
            Err(RecvTimeoutError::Timeout) => {
                self.retain(worker);
                Err(ResolveFailure::DeadlineExceeded)
            }
            Err(RecvTimeoutError::Disconnected) => {
                let _ = worker.join();
                Err(ResolveFailure::NotFound)
            }
        }
    }

    fn settle(&self) {
        // Reap what has finished. Settlement never blocks on a resolver the
        // platform will not interrupt; whatever is still running stays
        // registered and is reaped by a later call.
        let _ = self.pending();
    }
}

/// A resolver a test drives, with an explicit simulated cost per host.
///
/// The cost is charged to an injected [`ScriptedClock`], so a "slow DNS"
/// regression advances time by thirty seconds without waiting thirty seconds.
#[derive(Debug)]
pub struct ScriptedResolver {
    clock: std::sync::Arc<ScriptedClock>,
    entries: Mutex<Vec<ScriptedResolution>>,
}

#[derive(Clone, Debug)]
struct ScriptedResolution {
    host: String,
    cost: Duration,
    addresses: Vec<SocketAddr>,
}

impl ScriptedResolver {
    /// Create a resolver that charges its costs to `clock`.
    #[must_use]
    pub fn new(clock: std::sync::Arc<ScriptedClock>) -> Self {
        Self {
            clock,
            entries: Mutex::new(Vec::new()),
        }
    }

    /// Script one host: resolving it costs `cost` and yields `addresses`.
    #[must_use]
    pub fn with(self, host: &str, cost: Duration, addresses: Vec<SocketAddr>) -> Self {
        self.entries
            .lock()
            .expect("scripted resolver is not shared across a panic")
            .push(ScriptedResolution {
                host: host.to_owned(),
                cost,
                addresses,
            });
        self
    }
}

impl NameResolver for ScriptedResolver {
    fn resolve(
        &self,
        host: &str,
        _port: u16,
        budget: Duration,
    ) -> Result<Vec<SocketAddr>, ResolveFailure> {
        let entry = self
            .entries
            .lock()
            .expect("scripted resolver is not shared across a panic")
            .iter()
            .find(|entry| entry.host == host)
            .cloned();
        let Some(entry) = entry else {
            return Err(ResolveFailure::NotFound);
        };
        if entry.cost > budget {
            // Only the budget is actually spent: the caller stopped waiting.
            self.clock.advance(budget);
            return Err(ResolveFailure::DeadlineExceeded);
        }
        self.clock.advance(entry.cost);
        Ok(entry.addresses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network_provider::deadline::MonotonicClock as _;
    use std::net::Ipv4Addr;
    use std::sync::Arc;

    #[test]
    fn a_literal_address_resolves_without_a_worker_or_budget() {
        let resolver = SystemResolver::new();
        let resolved = resolver
            .resolve("127.0.0.1", 8080, Duration::ZERO)
            .expect("a literal address needs no name service");
        assert_eq!(
            resolved,
            vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080)]
        );
        assert_eq!(resolver.pending(), 0);
    }

    #[test]
    fn a_zero_budget_refuses_a_name_before_spawning_a_worker() {
        let resolver = SystemResolver::new();
        assert_eq!(
            resolver.resolve("localhost", 80, Duration::ZERO),
            Err(ResolveFailure::DeadlineExceeded)
        );
        assert_eq!(resolver.pending(), 0);
    }

    #[test]
    fn an_abandoned_worker_stays_owned_and_is_reaped() {
        let resolver = SystemResolver::new();
        // A one-nanosecond budget makes the caller give up essentially always.
        // Either outcome is admissible; what must hold is that no worker is
        // ever detached, whichever way the race lands.
        drop(resolver.resolve("localhost", 80, Duration::from_nanos(1)));
        for _ in 0..500 {
            if resolver.pending() == 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        resolver.settle();
        assert_eq!(
            resolver.pending(),
            0,
            "an abandoned resolver worker must be reaped, never leaked"
        );
    }

    #[test]
    fn a_scripted_resolution_charges_its_cost_to_the_clock() {
        let clock = Arc::new(ScriptedClock::new());
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80);
        let resolver = ScriptedResolver::new(clock.clone())
            .with("quick.invalid", Duration::from_millis(5), vec![address])
            .with("slow.invalid", Duration::from_secs(29), vec![address]);
        assert_eq!(
            resolver.resolve("quick.invalid", 80, Duration::from_secs(30)),
            Ok(vec![address])
        );
        assert_eq!(clock.elapsed_nanos(), Duration::from_millis(5).as_nanos());
        assert_eq!(
            resolver.resolve("slow.invalid", 80, Duration::from_secs(1)),
            Err(ResolveFailure::DeadlineExceeded)
        );
        assert_eq!(
            clock.elapsed_nanos(),
            Duration::from_millis(1_005).as_nanos()
        );
        assert_eq!(
            resolver.resolve("absent.invalid", 80, Duration::from_secs(30)),
            Err(ResolveFailure::NotFound)
        );
    }
}
