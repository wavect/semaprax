//! HP-03 host behaviour reachable through the public API (no grant needed).
//! Adapter-lifecycle cases need `Grant::issue` and live in `src/host/tests`.

use semaprax_harness::host::{
    AdapterState, ApprovedEndpoint, BudgetLedger, Credential, HostBudget, HttpClient, HttpLimits,
    InvocationClass,
};
use std::io::{Read, Write};
use std::net::TcpListener;

#[test]
fn hp03_remote_and_tls_endpoints_are_refused_with_an_explicit_code() {
    for url in [
        "http://example.org:80",
        "https://127.0.0.1:8443",
        "http://192.168.1.2:80",
    ] {
        let e = ApprovedEndpoint::from_host_config(url).unwrap_err();
        assert_eq!(e.code, "SPX-HPC030", "{url}");
        assert!(e.message.contains("requires TLS"), "{e}");
    }
}

#[test]
fn hp03_loopback_client_never_follows_redirects() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    let t = std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        let mut b = [0u8; 1024];
        let _ = s.read(&mut b);
        let _ = s.write_all(
            b"HTTP/1.1 301 Moved\r\nLocation: http://127.0.0.1:1/\r\nContent-Length: 0\r\n\r\n",
        );
    });
    let c = HttpClient::new(
        ApprovedEndpoint::from_host_config(&url).unwrap(),
        Some(Credential::new("X-Api-Key", "k").unwrap()),
        HttpLimits::default(),
    );
    assert_eq!(c.request("GET", "/", None).unwrap_err().code, "SPX-HPC032");
    t.join().unwrap();
}

#[test]
fn hp03_host_budget_is_its_own_ledger() {
    let b = HostBudget {
        max_jobs: 1,
        max_total_ms: 10,
        max_output_bytes: 10,
    };
    let mut l = BudgetLedger::default();
    l.start_job(&b).unwrap();
    l.finish_job(5, 5);
    assert_eq!(l.start_job(&b).unwrap_err().code, "SPX-HPC022");
}

#[test]
fn hp03_retry_boundary_and_state_names_are_stable() {
    assert!(InvocationClass::SafeRead.may_fall_back() && InvocationClass::Decision.may_fall_back());
    assert!(!InvocationClass::SideEffecting.may_fall_back());
    assert_eq!(AdapterState::Prepared.name(), "prepared");
    assert_eq!(AdapterState::Closed.name(), "closed");
}
