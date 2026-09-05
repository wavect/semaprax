//! Aggregate-deadline regressions for the real-socket provider.
//!
//! Every case here finishes in well under a second. The budget boundaries that
//! would otherwise take thirty seconds are reached by advancing an injected
//! [`ScriptedClock`] through a [`ScriptedResolver`], or by selecting a short
//! caller budget over a real loopback socket, never by waiting.

use std::io::{Read as _, Write as _};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::super::deadline::{DeadlinePolicy, ScriptedClock, MAX_OPERATION_DEADLINE};
use super::super::resolver::ScriptedResolver;
use super::super::{NetworkFailure, NetworkProvider as _, ProviderConnection, WaitState};
use super::TcpNetworkProvider;

/// The wall-clock ceiling every case in this module must stay under. It is far
/// below the thirty-second maximum, so a regression that reinstates a
/// per-sub-operation timeout fails here instead of hanging a gate.
const FAST: Duration = Duration::from_secs(5);

fn loopback(port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
}

/// A listener that records whether anything ever connected to it.
struct WitnessListener {
    port: u16,
    accepted: mpsc::Receiver<()>,
    _worker: std::thread::JoinHandle<()>,
}

impl WitnessListener {
    fn bind() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback bind");
        let port = listener.local_addr().expect("local address").port();
        let (sender, accepted) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            while listener.accept().is_ok() {
                if sender.send(()).is_err() {
                    break;
                }
            }
        });
        Self {
            port,
            accepted,
            _worker: worker,
        }
    }

    fn saw_a_connection(&self) -> bool {
        self.accepted.try_recv().is_ok()
    }
}

#[test]
fn a_caller_selected_budget_is_clamped_and_reported() {
    let provider = TcpNetworkProvider::new()
        .with_deadline_policy(DeadlinePolicy::new(Duration::from_secs(90)));
    assert_eq!(
        provider.deadline_policy().budget(),
        MAX_OPERATION_DEADLINE,
        "no host configuration may select an unbounded operation"
    );
    let provider = TcpNetworkProvider::new()
        .with_deadline_policy(DeadlinePolicy::new(Duration::from_millis(40)));
    assert_eq!(
        provider.deadline_policy().budget(),
        Duration::from_millis(40)
    );
}

#[test]
fn slow_name_resolution_consumes_the_whole_connect_budget() {
    // Resolution alone costs the entire thirty seconds. The connect must fail
    // on the aggregate bound; the scripted clock makes that instantaneous.
    let listener = WitnessListener::bind();
    let clock = Arc::new(ScriptedClock::new());
    let resolver = ScriptedResolver::new(clock.clone()).with(
        "slow.invalid",
        MAX_OPERATION_DEADLINE,
        vec![loopback(listener.port)],
    );
    let mut provider = TcpNetworkProvider::new()
        .with_deadline_policy(DeadlinePolicy::with_clock(
            MAX_OPERATION_DEADLINE,
            clock.clone(),
        ))
        .with_resolver(Arc::new(resolver));
    let started = Instant::now();
    assert_eq!(
        provider.connect("slow.invalid", listener.port),
        Err(NetworkFailure::ConnectFailed)
    );
    assert!(started.elapsed() < FAST, "the bound must not be waited out");
    assert!(
        !listener.saw_a_connection(),
        "a connect whose budget resolution consumed must attempt no address"
    );
    // Nothing was retained: no handle exists and settlement is clean.
    assert_eq!(
        provider.recv(ProviderConnection::new(0), 1),
        Err(NetworkFailure::UnknownHandle)
    );
    provider.settle();
}

#[test]
fn several_failing_addresses_cannot_multiply_the_connection_budget() {
    // Four candidate addresses, but resolution already spent all but one
    // millisecond of the budget. The first attempt therefore consumes what is
    // left and no address gets a fresh thirty seconds.
    let listener = WitnessListener::bind();
    let clock = Arc::new(ScriptedClock::new());
    let candidates = vec![
        loopback(listener.port),
        loopback(listener.port),
        loopback(listener.port),
        loopback(listener.port),
    ];
    let resolver = ScriptedResolver::new(clock.clone()).with(
        "many.invalid",
        MAX_OPERATION_DEADLINE,
        candidates,
    );
    let mut provider = TcpNetworkProvider::new()
        .with_deadline_policy(DeadlinePolicy::with_clock(
            MAX_OPERATION_DEADLINE,
            clock.clone(),
        ))
        .with_resolver(Arc::new(resolver));
    let started = Instant::now();
    assert_eq!(
        provider.connect("many.invalid", listener.port),
        Err(NetworkFailure::ConnectFailed)
    );
    assert!(
        started.elapsed() < FAST,
        "four addresses must share one budget, not restart it four times"
    );
    assert!(!listener.saw_a_connection());
    provider.settle();
}

#[test]
fn a_literal_address_still_connects_under_a_short_budget() {
    // The bound is a ceiling on waiting, never a reason to fail work that
    // completes. This is the real-adapter loopback case.
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback bind");
    let port = listener.local_addr().expect("local address").port();
    let peer = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut input = [0u8; 4];
        stream.read_exact(&mut input).expect("read");
        assert_eq!(&input, b"ping");
        stream.write_all(b"pong").expect("write");
    });
    let mut provider =
        TcpNetworkProvider::new().with_deadline_policy(DeadlinePolicy::new(Duration::from_secs(5)));
    let connection = provider
        .connect("127.0.0.1", port)
        .expect("loopback connect");
    assert_eq!(provider.send(connection, b"ping"), Ok(4));
    assert_eq!(provider.recv(connection, 4), Ok(b"pong".to_vec()));
    assert_eq!(provider.close(connection), Ok(()));
    provider.settle();
    peer.join().expect("peer thread");
}

#[test]
fn a_silent_peer_bounds_a_read_by_the_operation_deadline() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback bind");
    let port = listener.local_addr().expect("local address").port();
    // The peer connects and then says nothing at all until the test ends.
    let (release, held) = mpsc::channel::<()>();
    let peer = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept");
        let _ = held.recv();
        drop(stream);
    });
    let mut provider = TcpNetworkProvider::new()
        .with_deadline_policy(DeadlinePolicy::new(Duration::from_millis(150)));
    let connection = provider
        .connect("127.0.0.1", port)
        .expect("loopback connect");
    let started = Instant::now();
    assert_eq!(
        provider.recv(connection, 16),
        Err(NetworkFailure::TransferFailed)
    );
    assert!(
        started.elapsed() < FAST,
        "a silent peer must not hold a read for the per-syscall default"
    );
    provider.settle();
    drop(release);
    peer.join().expect("peer thread");
}

#[test]
fn a_wait_never_outlasts_the_operation_deadline() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback bind");
    let port = listener.local_addr().expect("local address").port();
    let (release, held) = mpsc::channel::<()>();
    let peer = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept");
        let _ = held.recv();
        drop(stream);
    });
    let mut provider = TcpNetworkProvider::new()
        .with_deadline_policy(DeadlinePolicy::new(Duration::from_millis(120)));
    let connection = provider
        .connect("127.0.0.1", port)
        .expect("loopback connect");
    let started = Instant::now();
    // The program asks for the full thirty-second readiness wait the language
    // admits; the operation deadline caps it anyway.
    assert_eq!(provider.wait(connection, 30_000), Ok(WaitState::Timeout));
    assert!(
        started.elapsed() < FAST,
        "the program's own timeout is a cap, not a licence to exceed the deadline"
    );
    provider.settle();
    drop(release);
    peer.join().expect("peer thread");
}

#[test]
fn a_peer_that_never_reads_bounds_a_partial_write_in_aggregate() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback bind");
    let port = listener.local_addr().expect("local address").port();
    // The peer accepts and then never reads a byte, so the socket buffers fill
    // and `send` makes progress one chunk at a time and then stalls.
    let (release, held) = mpsc::channel::<()>();
    let peer = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept");
        let _ = held.recv();
        drop(stream);
    });
    let mut provider = TcpNetworkProvider::new()
        .with_deadline_policy(DeadlinePolicy::new(Duration::from_millis(200)));
    let connection = provider
        .connect("127.0.0.1", port)
        .expect("loopback connect");
    let payload = vec![0u8; 32 * 1024 * 1024];
    let started = Instant::now();
    assert_eq!(
        provider.send(connection, &payload),
        Err(NetworkFailure::TransferFailed),
        "a stalled peer must not extend the operation past its budget"
    );
    assert!(
        started.elapsed() < FAST,
        "every partial write draws down one budget"
    );
    provider.settle();
    drop(release);
    peer.join().expect("peer thread");
}

#[test]
fn a_bounded_accept_stops_waiting_at_the_deadline() {
    let reservation = TcpListener::bind(("127.0.0.1", 0)).expect("loopback bind");
    let port = reservation.local_addr().expect("local address").port();
    drop(reservation);
    let mut provider = TcpNetworkProvider::new()
        .with_deadline_policy(DeadlinePolicy::new(Duration::from_millis(120)));
    let listener = provider.listen("127.0.0.1", port).expect("loopback listen");
    let started = Instant::now();
    assert_eq!(
        provider.accept(listener),
        Err(NetworkFailure::AcceptFailed),
        "an accept with no caller must end on the operation deadline"
    );
    assert!(started.elapsed() < FAST);
    // A later accept gets its own budget and still succeeds.
    let client = std::thread::spawn(move || {
        let mut attempts = 0;
        loop {
            if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
                let _ = stream.write_all(b"ping");
                return;
            }
            attempts += 1;
            assert!(attempts < 500, "loopback client could not connect");
            std::thread::sleep(Duration::from_millis(2));
        }
    });
    let connection = provider.accept(listener).expect("second accept");
    assert_eq!(provider.recv(connection, 4), Ok(b"ping".to_vec()));
    assert_eq!(provider.close_listener(listener), Ok(()));
    provider.settle();
    client.join().expect("client thread");
}
