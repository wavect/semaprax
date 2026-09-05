//! Loopback TLS rejection evidence for the real-socket provider.
//!
//! The sibling success case proves an authenticated peer transfers bytes.
//! These cases prove the two rejections that matter for an explicitly granted
//! client: a certificate no installed root vouches for, and a certificate that
//! does not name the host the program asked for. Both bind `127.0.0.1:0` and
//! talk only to their own listener thread.

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

use super::super::{NetworkFailure, NetworkProvider as _};
use super::TcpNetworkProvider;

/// A self-signed `CN=localhost` certificate that chains to nothing the client
/// trusts.
const SELF_SIGNED_CERT: &str = "MIIDHzCCAgegAwIBAgIUFIcDxjb6bqmjBssaRaZHxICzAbUwDQYJKoZIhvcNAQELBQAwFDESMBAGA1UEAwwJbG9jYWxob3N0MB4XDTI2MDkwNTE0NDkwNVoXDTM2MDkwMjE0NDkwNVowFDESMBAGA1UEAwwJbG9jYWxob3N0MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAomKQfiLXU09Rj6V2AlzcOsqnmbj+bHggdPg9wgBqHnRXmzvzCz8KpWk28KpKgpJ9lWi7g5UF8MNDy/0eWUNjnJwG2T/H7cUpj7F3OHF+hoJSH/N+I13CS1hvgrrpYjDDhMcW/D+3E6P7ayQsHnY9R4Y7Y2jOrZIXLke+3MbIQ5E4ry+tSbPW+BY+zh+kBRVEGPnXXB8mTJ5GZvbW+h0WMbWRDgZfI6iSsjPASmBpj7bgh8nOiMY5ryntTPsbf9rWdCrVMKL0jOlgJGLlUgjsS+bi1LpjCJh1lbbTRTrdemLujucQSsj+CKrvsL0SWsE8cv2IXsfK10aT3FYuoh4KuwIDAQABo2kwZzAdBgNVHQ4EFgQUxrBtA/xM4knpIdRnrOKM+ziffPMwHwYDVR0jBBgwFoAUxrBtA/xM4knpIdRnrOKM+ziffPMwDwYDVR0TAQH/BAUwAwEB/zAUBgNVHREEDTALgglsb2NhbGhvc3QwDQYJKoZIhvcNAQELBQADggEBAD2B86rFdIc2rpj+xIP4/brBhosRUT7P8BjkEN0rVFBZQW8VnasartEWFN+f6Z1QgVjF6/CMINj+ereFT15aKVnG6AQD7xOaXf1ipp/8Ni9OBV5NP6iL5qyrkcHjEjeziEBgltvrK8qLDtDJweQEoD1QHQDHcdLSwt2DExR2U/ADu0FykKlJ51qpWlvL4hPzAKgHUiXhDZBxIX5kFF2L2jXRJFeY8j/SgAHPC8pdaeUorjVSxML92BPGf8BCf3ktnQSGnJX8Mvs4KdzEejJNPA/2vosddP+z1Al2nCQdCjtzsgsrku0Tg8g5CUuNy14gy85z9whd0I1/uk4VPdccZAA=";
const SELF_SIGNED_KEY: &str = "MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQCiYpB+ItdTT1GPpXYCXNw6yqeZuP5seCB0+D3CAGoedFebO/MLPwqlaTbwqkqCkn2VaLuDlQXww0PL/R5ZQ2OcnAbZP8ftxSmPsXc4cX6GglIf834jXcJLWG+CuuliMMOExxb8P7cTo/trJCwedj1HhjtjaM6tkhcuR77cxshDkTivL61Js9b4Fj7OH6QFFUQY+ddcHyZMnkZm9tb6HRYxtZEOBl8jqJKyM8BKYGmPtuCHyc6IxjmvKe1M+xt/2tZ0KtUwovSM6WAkYuVSCOxL5uLUumMImHWVttNFOt16Yu6O5xBKyP4Iqu+wvRJawTxy/Yhex8rXRpPcVi6iHgq7AgMBAAECggEADTb4NfZKj6e6JiEmTrWVNojyGrsd+WB37mJFTwRkSRYutZ4AqWmintNxJSS2lj8ATqhh734GfbwQ8wjQ73K3KIeKByP+9oU/sfHp7INP80z9DJyZfJyksyep63mf2d3ItjAoLrRWBxh73WbphZEZwOMA8kC/5mAn2BxT6/jr/ejc+LqvAXLgGd0JDAtCupgri7XK8lCXSwk36yMSvGdPFAR2QxpdQ4xl65KYI/uHG/rODWyayKKkDHJB3q0BGp3n8uo33R1YPHrgJIxrdlLB1FOt1a69+xeGi6aeikH9TJ0dD/FvfI+7f4J5MO/cCpPkgqv+DXuuW3lVi8ZbN9woAQKBgQDXAROPJHwJvCD5NPUL/wWFgSqOZ3kvNtl/l7YUbP4IxFzzd3QTVQsl+72Wkjhd+GaEM8TX3rSZDtjLJElfNpIhztGb6mc5oSrHab5EVY1CKxFwXSJjm9XTcfvtV74oKwfPOAGu0/8ReZqqRPdOLpXJbmmGXSynmwRF+LS0EDGJmQKBgQDBWQGibVHW3N0Rc1PeZgzjyGKtbjTdLtUEzvs65XidAu+slcX9kj9GL6cjAPC6lzjW0C4KowZEQQnKTmFcsD5XrbFISEmNpjgaSu+i5xCFb6xiJVN4S/vmFUYvW4qudwxSbLpmbJYUyYdn0TnhHI6Qp6BC1NnNIcceijo7KunzcwKBgGe/VTjVWiU4apDWRQis3nU1htuAgrGNvhYblvj0PwDsAA5brd9GQkLp3uoxVJHDs3RHpsyj4nGZAHPF5sHTC2DU88BQs87TPllLZUyEG826Cog16Mo4AE4vymkU1eV8HiCX3fgGxCYij8dp0Awh3pV8ed8kRs/5tW4uPMRGrCDBAoGATXFjMDXtU8x/V6AD9c6WVx5KOAEud3FsrVJiWoLTPsCQU2ZiOWC8q1Ym8eRMh9BOWexkpKoLtob+buPaJ5AISIIvwi4CGBR94Donpe47Ndc3CtC8kDCPIudeh1V5RMw2SUV3m9LeglD+RV0Oe9Y+XD5n+Jzc6EchRGBFVrGnp3UCgYAcWyNY04NlMX8jVkRMhXHChicIMKuUFER4VCdF48fdafra4nfsst5GHSufRyzfW74mEJTeSJFp5im5cAvbEClrsDM8scScXKbtirBmE7eQYK8pOFTvl2svJX0XsL5G5xbouw5tMvH27Nfx16PL84B4oo4r5cOCDTLW+rI7PFvPsQ=";
/// The private test CA and the `CN=localhost` leaf it issued.
const ROOT: &str = "MIIDJzCCAg+gAwIBAgIUC3kI/KYpwSCFZIOpQLwZZv3fpIUwDQYJKoZIhvcNAQELBQAwGzEZMBcGA1UEAwwQU0VNQVBSQVggVGVzdCBDQTAeFw0yNjA5MDUxNDU3MTlaFw0zNjA5MDIxNDU3MTlaMBsxGTAXBgNVBAMMEFNFTUFQUkFYIFRlc3QgQ0EwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQCtxpzwCk3e4aRY3ozKBTi94gfLHe6yKDfDggOHGiwUGotJ9dVH8e4Hh82JamO+jH694HBmjlbGXF+BY7Gxv/Vz8Z7R9VqS1uND7J4V4pJABLL4H//k/c0WPMopTkQRmVyit34hTob14aL+hPq4DFOtH+FxXiUyPaJp6xP0UH7KTJpSBJfBlTAmJoBuMP7Ara05oozrVuLNzSDaUulGGkA5kUuv2GnPvQjTx8PG14GUfJt6okOD64JJSaoQCrraxyHIG8UmZgnHyoIq3UgFY9gj4haVW6ykKe+bkWVbwCOZcMAffzx+NKDodSahn3Qy2z0eDI0ARMtVFDE+ijtxlG/1AgMBAAGjYzBhMB0GA1UdDgQWBBT4Dg/tRse2xlFPUoKfa/7M5c40VjAfBgNVHSMEGDAWgBT4Dg/tRse2xlFPUoKfa/7M5c40VjAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwIBBjANBgkqhkiG9w0BAQsFAAOCAQEAmEWc71S2305pR9Ps29VDVdwOcVoetWsqEnCsAIHg0qfioQz3mznfxE3gOZ4gm03AOslf2sqq8ev02MnEuZWt7Y7xwstrTyo0EA4mWXzBTz0EX7Qp1PgV4MV7Lifp+Dv5ACDx75bgOziKx+u6VVvR0RoE1tUB3m3ihO7aT0HMXOBvElkuY7Ev+fR7lgSFOPGYV2IIBcfaro0dGJlixyBjP/TLGAr8S6buf0ZFCBKtMriXyfiqcQ8IPeLEOtFGxhrWKoNoRpkYwM5kut27vDkoc5UekFmU4EaGPl0cWEpoky5RMXgrA0hAzKEmgPnbIVplKwdoELQjon+MR1HA9txCeg==";
const LEAF: &str = "MIIDSjCCAjKgAwIBAgIUK81c/KylyZTx6OJ/K9lJP7OLzBgwDQYJKoZIhvcNAQELBQAwGzEZMBcGA1UEAwwQU0VNQVBSQVggVGVzdCBDQTAeFw0yNjA5MDUxNDU3MTlaFw0zNjA5MDIxNDU3MTlaMBQxEjAQBgNVBAMMCWxvY2FsaG9zdDCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAM6ibgX7OJCn5nsP0DH497ZCdsxQN23ifpv3ZWWNbKScZi4k5R0nZqJb/asrOa/vgc/An5YBYdsHV/9SqE7CVxhgCj+sYo6W2RfyDV8PF3fztxg+1Varrm0RcI4DaZN2N7fqdxZPvpIl//3n3J2G6J2d919ZPZpog0ahqlHjfvmIh1ESeS2XIu1T4dHlBvW1m3AgoFneNZDHDQs9ziuKte6KShv2I6rOzIRSC5vHM4YsDC64NANbheAV0L98rc/51A6jJxziKQtpFDhBHGvAhag3JkOUyLP7fiIPiHBI0Qxmh70EBj2EgUo5OqV1pNytbH4zBrKlyjQj+R2o8ReNpY8CAwEAAaOBjDCBiTAUBgNVHREEDTALgglsb2NhbGhvc3QwDAYDVR0TAQH/BAIwADAOBgNVHQ8BAf8EBAMCBaAwEwYDVR0lBAwwCgYIKwYBBQUHAwEwHQYDVR0OBBYEFD69svZnO8+sMQfesN19Zk40CBU8MB8GA1UdIwQYMBaAFPgOD+1Gx7bGUU9Sgp9r/szlzjRWMA0GCSqGSIb3DQEBCwUAA4IBAQAwcYsnw9zK+9lMrIN6zSxry26FFIjOP/ZRXSeloNPA2Fd2p+16b7RoHL+tcn4P4NMCKsz2Y+faX6lzSzIi0lydRsM8rH3xY4/Y8UDoLyC6zDQXpZNbEyWQALgKoZjV8l4XEbtmhLx++h2wArD/eEneBW3aCL8QzNgTU6gyobp1y6AqxQPnl+2SpBlFtpnoz0W3CCOGc0UiaobxBNTYydtY37vGQPLs32drQ2E0o9RfD+4/MTTkS380fXI4pEW4XOm/AofuMwVz1zkWXY/CzYp+1czf7/sOLDTsuwt0/QJFhK3IGSBL1wH3lU8BUHC6LMysilY3Eujo+Ya7dHAyM0lb";
const LEAF_KEY: &str = "MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQDOom4F+ziQp+Z7D9Ax+Pe2QnbMUDdt4n6b92VljWyknGYuJOUdJ2aiW/2rKzmv74HPwJ+WAWHbB1f/UqhOwlcYYAo/rGKOltkX8g1fDxd387cYPtVWq65tEXCOA2mTdje36ncWT76SJf/959ydhuidnfdfWT2aaINGoapR4375iIdREnktlyLtU+HR5Qb1tZtwIKBZ3jWQxw0LPc4rirXuikob9iOqzsyEUgubxzOGLAwuuDQDW4XgFdC/fK3P+dQOoycc4ikLaRQ4QRxrwIWoNyZDlMiz+34iD4hwSNEMZoe9BAY9hIFKOTqldaTcrWx+Mwaypco0I/kdqPEXjaWPAgMBAAECggEAS9lKyq5HOq4vB8Aru5Q4lXH7Oo89cXwA3o5m7WqG1TvFtC193oA+h919lW3F/KNNgq2hxsXWHjipYAL+3f4vSzbBvFKyUMXlhYknyFt5UWIoNOGnnOtjGQ0cRDzTbbooxL1vnkSCXxJMz+5iyH4jd+vqyFixKLMxcOVZ6Do6OyzuFK2hq1dp2R+fk0TVyQAFTtqSVC5DR/dxzX+mIkkzJWJvfsTnlBZ19j9q8ft0XnOfEpHDSfxzoOXx1SdF+CvA15kjmWVUQbHTMgcPni90NhomPgdlhqXfHx+N+ar3GJO9+GJ8QGhwPXGRGpa81lkQZMTb0Q+rsbqws3Xvl1Nz4QKBgQDtKB7jWevWtakv6k8i6HVe4iGxBwYAHUKe8IrMZt5HQ0gs4iBU6kwZtgW9c02VeHYHnSf/oEF/2OnXpxyQjiHR5LkcZ87lnuivX0bZo8Ijt1dXfczQFZA/zCfpuoTHSQKD8Mw5MbrQ1XrRZaYZMlZ6f0OBPMN8P1657nVwCg3RIQKBgQDfDXj8HqC2blafwwb2dUvKQSH7J4biz7QFl/ZTCJyEu8SSLNJRnKyrIC5mewdJFM3CT9eqIklNkrxbIqd0URy0i512cVIjQmGTtaD0c3S361N9MStlKwsrCtj7Oy4qBdlq/lG03pMubWntRdXnm6e+l+KG6fZ+h+W5y6MEXLWwrwKBgHsfISoXPQEzPqrJklwlIwonjCZD5zGX/0ZUyzpjDXMh0w66Nt7e5LNUdJZujhDTgTNiu6lSoa6mBoEXGRVTNOurOw8sNZWwckzZwgarpda1EHszrGk7SLBWZUJKuzRbCxtEoEHxN3PD4QdlJl5ea9ccywcFbNfMbnlI+183WQUBAoGAVyqBrC0f6wsFiRuC/g9qldiMOgUBXmOC22i+V0aXO/vQ3rrrWf9bLui9mUjc2P9rRVNEWXVaphkAyLCrNfZ4vEmPOHkieyr2zO1+v+japQEuuE7dwYRnseNkVhGTgdKVW42VSpRseglCCvpulDss+3uJh+WocVwUN15QD2VXj3sCgYAyP2FCNPdfg1r2LcNMn06gwnLz+NHn4HK1PNjrRTQgrKYG9xf8gvM0HgoSdR1mfDjdPqgPMdLFG23jmpOG23waokgIsBl88SGdaCVJ/+Ti4WFHhKkhRwgmNX/4se+JsD5nSGaBwkrZ6uyLs+W39hFa0MQzDdRCQjsuuRWFsn7YpA==";

fn decode64(input: &str) -> Vec<u8> {
    fn digit(byte: u8) -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let mut output = Vec::new();
    let mut bits = 0u32;
    let mut count = 0u8;
    for byte in input.bytes().filter(|byte| *byte != b'=') {
        bits = (bits << 6) | u32::from(digit(byte).expect("test fixture is base64"));
        count += 6;
        if count >= 8 {
            count -= 8;
            output.push((bits >> count) as u8);
            bits &= (1u32 << count) - 1;
        }
    }
    output
}

fn crypto() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// A client that trusts exactly the private test CA and nothing else.
fn client_trusting_the_test_root() -> rustls::ClientConfig {
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(decode64(ROOT)))
        .expect("the test root is a certificate");
    rustls::ClientConfig::builder_with_provider(crypto())
        .with_safe_default_protocol_versions()
        .expect("ring has safe protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth()
}

/// A loopback TLS server presenting `cert`/`key`, which runs one handshake and
/// then goes away. A rejected handshake is an expected outcome, so nothing
/// here asserts the server side succeeded.
fn serve_once(cert: &str, key: &str) -> (u16, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback bind");
    let port = listener.local_addr().expect("local address").port();
    let certificate = rustls::pki_types::CertificateDer::from(decode64(cert));
    let private = rustls::pki_types::PrivatePkcs8KeyDer::from(decode64(key));
    let config = rustls::ServerConfig::builder_with_provider(crypto())
        .with_safe_default_protocol_versions()
        .expect("ring has safe protocol versions")
        .with_no_client_auth()
        .with_single_cert(vec![certificate], private.into())
        .expect("the test key matches its certificate");
    let worker = std::thread::spawn(move || {
        if let Ok((socket, _)) = listener.accept() {
            if let Ok(connection) = rustls::ServerConnection::new(Arc::new(config)) {
                let mut stream = rustls::StreamOwned::new(connection, socket);
                let mut scratch = [0u8; 1];
                let _ = stream.read(&mut scratch);
                let _ = stream.write(&scratch);
            }
        }
    });
    (port, worker)
}

#[test]
fn an_untrusted_root_is_rejected_and_issues_no_handle() {
    let (port, server) = serve_once(SELF_SIGNED_CERT, SELF_SIGNED_KEY);
    let mut provider =
        TcpNetworkProvider::with_tls_config(Arc::new(client_trusting_the_test_root()));
    assert_eq!(
        provider.connect_tls("localhost", port),
        Err(NetworkFailure::TlsFailed),
        "a certificate no installed root vouches for must not authenticate"
    );
    // A rejected handshake publishes nothing: the program never received a
    // handle, so the very first token is still unknown.
    assert_eq!(
        provider.recv(super::super::ProviderConnection::new(0), 1),
        Err(NetworkFailure::UnknownHandle)
    );
    provider.settle();
    // Unblock the server if it is still waiting for a client that gave up.
    let _ = TcpStream::connect(("127.0.0.1", port));
    let _ = server.join();
}

#[test]
fn a_certificate_that_does_not_name_the_host_is_rejected() {
    // The leaf is issued by the trusted test root and names `localhost`. The
    // program asks for `127.0.0.1`, which the certificate does not cover.
    let (port, server) = serve_once(LEAF, LEAF_KEY);
    let mut provider =
        TcpNetworkProvider::with_tls_config(Arc::new(client_trusting_the_test_root()));
    assert_eq!(
        provider.connect_tls("127.0.0.1", port),
        Err(NetworkFailure::TlsFailed),
        "a trusted chain is not enough; the name must match too"
    );
    assert_eq!(
        provider.recv(super::super::ProviderConnection::new(0), 1),
        Err(NetworkFailure::UnknownHandle)
    );
    provider.settle();
    let _ = TcpStream::connect(("127.0.0.1", port));
    let _ = server.join();
}
