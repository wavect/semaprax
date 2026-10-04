//! Acceptance for the runnable reference-service host: a real server
//! process serves login/CRUD/job routes from the scaffold's checked `.spx`
//! decisions with physical persistence, survives kill-and-restart without
//! duplicating settled effects, and refuses fixture-mode intent.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use semaprax::network_provider::{client_tls_config_trusting, NetworkProvider, TcpNetworkProvider};
#[path = "reference_service_acceptance/local_provider.rs"]
mod local_provider;

const SERVER: &str = env!("CARGO_BIN_EXE_semaprax-reference-service");
const READY_TIMEOUT: Duration = Duration::from_secs(300);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Canonical host-mode service configuration (sorted keys plus LF, exactly
/// as `service_config::decode` requires). Origins use the `.invalid` TLD so
/// no real peer can exist; the delivery attempt fails closed by design.
const HOST_CONFIG: &str = "{\"database\":{\"adapter\":\"snapshot\",\"dsn_secret_ref\":null,\"migration_table\":\"semaprax_migrations\"},\"http\":{\"adapter\":\"native\",\"listen_origin\":\"https://service.invalid\",\"tls_profile\":\"modern\"},\"mode\":\"host\",\"schema\":\"semaprax.service-config.v1\",\"secrets\":{\"password_pepper_ref\":\"auth.pepper\",\"session_signing_key_ref\":\"auth.session\",\"webhook_signing_key_ref\":\"webhook.signing\"},\"telemetry\":{\"adapter\":\"semaprax-json-events\",\"endpoint_origin\":\"https://telemetry.invalid:9\"}}\n";

/// Test-only TLS material for the server's held certificate/key secrets:
/// `CN=localhost`, issued by a private test CA. Neither authenticates any
/// production identity; this is the same fixture already carried by the
/// root crate's `network_provider::tcp` TLS unit tests.
const TLS_ROOT: &str = "MIIDJzCCAg+gAwIBAgIUC3kI/KYpwSCFZIOpQLwZZv3fpIUwDQYJKoZIhvcNAQELBQAwGzEZMBcGA1UEAwwQU0VNQVBSQVggVGVzdCBDQTAeFw0yNjA5MDUxNDU3MTlaFw0zNjA5MDIxNDU3MTlaMBsxGTAXBgNVBAMMEFNFTUFQUkFYIFRlc3QgQ0EwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQCtxpzwCk3e4aRY3ozKBTi94gfLHe6yKDfDggOHGiwUGotJ9dVH8e4Hh82JamO+jH694HBmjlbGXF+BY7Gxv/Vz8Z7R9VqS1uND7J4V4pJABLL4H//k/c0WPMopTkQRmVyit34hTob14aL+hPq4DFOtH+FxXiUyPaJp6xP0UH7KTJpSBJfBlTAmJoBuMP7Ara05oozrVuLNzSDaUulGGkA5kUuv2GnPvQjTx8PG14GUfJt6okOD64JJSaoQCrraxyHIG8UmZgnHyoIq3UgFY9gj4haVW6ykKe+bkWVbwCOZcMAffzx+NKDodSahn3Qy2z0eDI0ARMtVFDE+ijtxlG/1AgMBAAGjYzBhMB0GA1UdDgQWBBT4Dg/tRse2xlFPUoKfa/7M5c40VjAfBgNVHSMEGDAWgBT4Dg/tRse2xlFPUoKfa/7M5c40VjAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwIBBjANBgkqhkiG9w0BAQsFAAOCAQEAmEWc71S2305pR9Ps29VDVdwOcVoetWsqEnCsAIHg0qfioQz3mznfxE3gOZ4gm03AOslf2sqq8ev02MnEuZWt7Y7xwstrTyo0EA4mWXzBTz0EX7Qp1PgV4MV7Lifp+Dv5ACDx75bgOziKx+u6VVvR0RoE1tUB3m3ihO7aT0HMXOBvElkuY7Ev+fR7lgSFOPGYV2IIBcfaro0dGJlixyBjP/TLGAr8S6buf0ZFCBKtMriXyfiqcQ8IPeLEOtFGxhrWKoNoRpkYwM5kut27vDkoc5UekFmU4EaGPl0cWEpoky5RMXgrA0hAzKEmgPnbIVplKwdoELQjon+MR1HA9txCeg==";
const TLS_LEAF: &str = "MIIDSjCCAjKgAwIBAgIUK81c/KylyZTx6OJ/K9lJP7OLzBgwDQYJKoZIhvcNAQELBQAwGzEZMBcGA1UEAwwQU0VNQVBSQVggVGVzdCBDQTAeFw0yNjA5MDUxNDU3MTlaFw0zNjA5MDIxNDU3MTlaMBQxEjAQBgNVBAMMCWxvY2FsaG9zdDCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAM6ibgX7OJCn5nsP0DH497ZCdsxQN23ifpv3ZWWNbKScZi4k5R0nZqJb/asrOa/vgc/An5YBYdsHV/9SqE7CVxhgCj+sYo6W2RfyDV8PF3fztxg+1Varrm0RcI4DaZN2N7fqdxZPvpIl//3n3J2G6J2d919ZPZpog0ahqlHjfvmIh1ESeS2XIu1T4dHlBvW1m3AgoFneNZDHDQs9ziuKte6KShv2I6rOzIRSC5vHM4YsDC64NANbheAV0L98rc/51A6jJxziKQtpFDhBHGvAhag3JkOUyLP7fiIPiHBI0Qxmh70EBj2EgUo5OqV1pNytbH4zBrKlyjQj+R2o8ReNpY8CAwEAAaOBjDCBiTAUBgNVHREEDTALgglsb2NhbGhvc3QwDAYDVR0TAQH/BAIwADAOBgNVHQ8BAf8EBAMCBaAwEwYDVR0lBAwwCgYIKwYBBQUHAwEwHQYDVR0OBBYEFD69svZnO8+sMQfesN19Zk40CBU8MB8GA1UdIwQYMBaAFPgOD+1Gx7bGUU9Sgp9r/szlzjRWMA0GCSqGSIb3DQEBCwUAA4IBAQAwcYsnw9zK+9lMrIN6zSxry26FFIjOP/ZRXSeloNPA2Fd2p+16b7RoHL+tcn4P4NMCKsz2Y+faX6lzSzIi0lydRsM8rH3xY4/Y8UDoLyC6zDQXpZNbEyWQALgKoZjV8l4XEbtmhLx++h2wArD/eEneBW3aCL8QzNgTU6gyobp1y6AqxQPnl+2SpBlFtpnoz0W3CCOGc0UiaobxBNTYydtY37vGQPLs32drQ2E0o9RfD+4/MTTkS380fXI4pEW4XOm/AofuMwVz1zkWXY/CzYp+1czf7/sOLDTsuwt0/QJFhK3IGSBL1wH3lU8BUHC6LMysilY3Eujo+Ya7dHAyM0lb";
const TLS_LEAF_KEY: &str = "MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQDOom4F+ziQp+Z7D9Ax+Pe2QnbMUDdt4n6b92VljWyknGYuJOUdJ2aiW/2rKzmv74HPwJ+WAWHbB1f/UqhOwlcYYAo/rGKOltkX8g1fDxd387cYPtVWq65tEXCOA2mTdje36ncWT76SJf/959ydhuidnfdfWT2aaINGoapR4375iIdREnktlyLtU+HR5Qb1tZtwIKBZ3jWQxw0LPc4rirXuikob9iOqzsyEUgubxzOGLAwuuDQDW4XgFdC/fK3P+dQOoycc4ikLaRQ4QRxrwIWoNyZDlMiz+34iD4hwSNEMZoe9BAY9hIFKOTqldaTcrWx+Mwaypco0I/kdqPEXjaWPAgMBAAECggEAS9lKyq5HOq4vB8Aru5Q4lXH7Oo89cXwA3o5m7WqG1TvFtC193oA+h919lW3F/KNNgq2hxsXWHjipYAL+3f4vSzbBvFKyUMXlhYknyFt5UWIoNOGnnOtjGQ0cRDzTbbooxL1vnkSCXxJMz+5iyH4jd+vqyFixKLMxcOVZ6Do6OyzuFK2hq1dp2R+fk0TVyQAFTtqSVC5DR/dxzX+mIkkzJWJvfsTnlBZ19j9q8ft0XnOfEpHDSfxzoOXx1SdF+CvA15kjmWVUQbHTMgcPni90NhomPgdlhqXfHx+N+ar3GJO9+GJ8QGhwPXGRGpa81lkQZMTb0Q+rsbqws3Xvl1Nz4QKBgQDtKB7jWevWtakv6k8i6HVe4iGxBwYAHUKe8IrMZt5HQ0gs4iBU6kwZtgW9c02VeHYHnSf/oEF/2OnXpxyQjiHR5LkcZ87lnuivX0bZo8Ijt1dXfczQFZA/zCfpuoTHSQKD8Mw5MbrQ1XrRZaYZMlZ6f0OBPMN8P1657nVwCg3RIQKBgQDfDXj8HqC2blafwwb2dUvKQSH7J4biz7QFl/ZTCJyEu8SSLNJRnKyrIC5mewdJFM3CT9eqIklNkrxbIqd0URy0i512cVIjQmGTtaD0c3S361N9MStlKwsrCtj7Oy4qBdlq/lG03pMubWntRdXnm6e+l+KG6fZ+h+W5y6MEXLWwrwKBgHsfISoXPQEzPqrJklwlIwonjCZD5zGX/0ZUyzpjDXMh0w66Nt7e5LNUdJZujhDTgTNiu6lSoa6mBoEXGRVTNOurOw8sNZWwckzZwgarpda1EHszrGk7SLBWZUJKuzRbCxtEoEHxN3PD4QdlJl5ea9ccywcFbNfMbnlI+183WQUBAoGAVyqBrC0f6wsFiRuC/g9qldiMOgUBXmOC22i+V0aXO/vQ3rrrWf9bLui9mUjc2P9rRVNEWXVaphkAyLCrNfZ4vEmPOHkieyr2zO1+v+japQEuuE7dwYRnseNkVhGTgdKVW42VSpRseglCCvpulDss+3uJh+WocVwUN15QD2VXj3sCgYAyP2FCNPdfg1r2LcNMn06gwnLz+NHn4HK1PNjrRTQgrKYG9xf8gvM0HgoSdR1mfDjdPqgPMdLFG23jmpOG23waokgIsBl88SGdaCVJ/+Ti4WFHhKkhRwgmNX/4se+JsD5nSGaBwkrZ6uyLs+W39hFa0MQzDdRCQjsuuRWFsn7YpA==";

/// A second, independently generated test CA + expired leaf pair used only
/// to prove expired-certificate rejection. Its whole validity window
/// (2020-01-01 to 2020-01-02) is in the past; it authenticates nothing.
const EXPIRED_CA_ROOT: &str = "MIIDNzCCAh+gAwIBAgIUEgSMffusg4l67BcMNc3DgI0FkxYwDQYJKoZIhvcNAQELBQAwIzEhMB8GA1UEAwwYU0VNQVBSQVggRXhwaXJlZCBUZXN0IENBMB4XDTE5MDEwMTAwMDAwMFoXDTIwMDYwMTAwMDAwMFowIzEhMB8GA1UEAwwYU0VNQVBSQVggRXhwaXJlZCBUZXN0IENBMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEArx+Y7thIy3gywZndV3cVIle/VA0tKIIxJRc173sQivXnr52a7WGLjI6TW8V/jWSligI9YyxDNAr1GJHrvrjlu6f6cNr186yBU45YyGVwyZmYRN4LdqGqisdyx2u36klUrYi2i4p6cIB9NheeGYTXGptbNk8DPUThWUcOmkIfs9ryf7y7nZ105RvOvVzHyRFh0eFHbxoiUsxoHYdJY0bYslozTaSJuqN4w27LPPWZ1agkayPkZawoK0Z7AJb6lhWm/kNnlNyyKtiCeY+s9rUthV9BJSI0C0RAWcNbaw5Y3aNdgbVoAHxlzkXBLaEEsWCR8pTQCQdP0yaoLaP2UhrJ4QIDAQABo2MwYTAdBgNVHQ4EFgQUl5KoJUpYKcgs8TexajlHCaMqoFwwHwYDVR0jBBgwFoAUl5KoJUpYKcgs8TexajlHCaMqoFwwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMCAQYwDQYJKoZIhvcNAQELBQADggEBABQHlLpbTn/5yRGeI1MlkyJo+BWqNJtw1w47Xdtq9C5e9v2Ne3ESOwFEZ/9XjE/hCbAEFXLDO20jT22KJb7oy/jKkzH99d6XjMqWthykPHBiBYPkG62GGMpaoR8h78kbnvVRYXHJAH0XsPdtoo0gKMV4j6rPyEEab/WMOhN/TvWw1lob54YNnYZXFdt+t/wFT9/La8rRntzlVH01OvB8Z79JWFW3p5VrHYt5KN9M4AL0jphKUWhyXPYSyqggx+C1l0HSkXGNRVizFhGWnYbHIdegPCuEdA9h3AAQ5yI9uNNtq7Ljn5La69WzoM4UuOhQ0r7fv2iH4zk/4eB+vh+Pw3M=";
const EXPIRED_LEAF: &str = "MIIDUjCCAjqgAwIBAgIUSeIvjA/kvcvQlJ7xRyCCkDWq8yswDQYJKoZIhvcNAQELBQAwIzEhMB8GA1UEAwwYU0VNQVBSQVggRXhwaXJlZCBUZXN0IENBMB4XDTIwMDEwMTAwMDAwMFoXDTIwMDEwMjAwMDAwMFowFDESMBAGA1UEAwwJbG9jYWxob3N0MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAmBzvifEFdZf+MFHg4+zH6V3E8pbIhYhKQpQ1flEAsKsykwlivNFOMd6sryT2h03KdES88H2X1QBiiGa8tNOyac3X964UJtqaUED9uU8p0gUVjxwodNfKlghQ5XBvmauvF9DwX6SuOSgGFZgihl0Y9Aammm//VeyjY85qbrQiZVywwG3BzmNTZ2/diZSqcgDKKpCVv8EHktY/0+q5Bh1oL9V6H4b5i1jsdkyqpHFBM85ipCs50vNEumK571ouRrcwaqSUgqjSZM7scZd56pMOq4Gpvb19N7JlRoJ9BS2kh9NhA0oy08ybrNEtE3YSd0H9B9Pp0k/69/2yaAyTpv/SpwIDAQABo4GMMIGJMBQGA1UdEQQNMAuCCWxvY2FsaG9zdDAMBgNVHRMBAf8EAjAAMA4GA1UdDwEB/wQEAwIFoDATBgNVHSUEDDAKBggrBgEFBQcDATAdBgNVHQ4EFgQUc+BAhNxtWwptT3kcjr3v+l1AqZwwHwYDVR0jBBgwFoAUl5KoJUpYKcgs8TexajlHCaMqoFwwDQYJKoZIhvcNAQELBQADggEBAHlHWhekl/yJpiG74buiYXHXMBJXUyq5/UjCWk4zP8u1qgFLyb4xwS03BxPI3LCD9CxkztbpfgC/KJwlxd33SrBzadgjJjkcWRnPLDYDvLCA8gHLn56+hVkvpygtQySdS0Ng3HKAYj9NT9GnczrbBxclBlKxnQ0Aq97o6bARYYxBmvUhARtS9yEjmc/Nw7p5G1ucQAd/q+6tO+XWOdjVuWxFAcy3TqdHhoNGJY1PyjxL7DORFWX4VA72Mkp1FWDpo4OjgG5W6FYEn2/rsR/lKnVqpPD9TECnlt2eCyRmN+AHKz4oYnQPy7vcVmDvn3898VvDZYquLqjCwMUVoNbdBc8=";
const EXPIRED_LEAF_KEY: &str = "MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQCYHO+J8QV1l/4wUeDj7MfpXcTylsiFiEpClDV+UQCwqzKTCWK80U4x3qyvJPaHTcp0RLzwfZfVAGKIZry007Jpzdf3rhQm2ppQQP25TynSBRWPHCh018qWCFDlcG+Zq68X0PBfpK45KAYVmCKGXRj0Bqaab/9V7KNjzmputCJlXLDAbcHOY1Nnb92JlKpyAMoqkJW/wQeS1j/T6rkGHWgv1XofhvmLWOx2TKqkcUEzzmKkKznS80S6YrnvWi5GtzBqpJSCqNJkzuxxl3nqkw6rgam9vX03smVGgn0FLaSH02EDSjLTzJus0S0TdhJ3Qf0H0+nST/r3/bJoDJOm/9KnAgMBAAECggEAKYUIazYDH/h3VPgccwo//PZv2imHHU+4uVicC1kP36kzGkhfD5vwBJO7vejQc9krcDYM/nXBmk3LF2E3lAIOumuJzhzRelOD+HDs8IZnq2Bg5Jmyf0YhkXc+oYnhpGfk2JLa8bhRJ9/BXWaT0eoadA1Wr2PvpaP8azM+AO6hTtokyg5BI6xQIKdEDaf/1q9Ypbcb7Wz1vl6X9Wrk8JXqQW4JCLC1l6dofvb/fll41enG3Uyn/mwCm9ztfijYiwRNGU8c6FZ+9dv7LVLjRAIim2vZdq16wJs1JjFGHQghhRtfXg17yJd9ZU9xmD2AUh6xXs6/Xi/yRDe1BVT1YxMxOQKBgQDL0DmFkXro4zjYoJKSyLL5vg3xMtZ+eQyFnMKcIYqQhQURwia4fKcDTJN9qj6VFbV3HHMQPECpaW7JrZwfRFvPn4cBJ3YDLD92X9cou6fzRxYAfGz8IKihf05q3RPtn32+mqs+mt/aGQ34jFJlbRqBq68ORrBjqZShyCZaf3/niQKBgQC/D81EC6he4dhmYB+69cV9v8RthHtEH/GqEpgymHhw8FqFvA2gU6qLs1Rgr0fxWu5mAjaF8yyp5pu4oRObKihfXFqARCjLc7yXC1xKPnYfAxmgMFXfMJy71kfv67DWJuVhXBRer990UqSo9V/nAQ1krzIHOvLS33kRd+hqtl0srwKBgQCMLPAC72XLWsu0IevtTF/b6F0KcN6ZKYP1OTWX0HHOp84uwouDAyiS2k3udfKI8t9VxplUpzwJyFvMFb10u70xdRSTNKKz1/Dl51DB0R7X8SIuv2Ttm0CfokE6ukaEfdcsCpCQhFBFXkn/kfLxkzJR0NSbSv7x7KYvBstqHprHkQKBgDFosLiMGzqORRwUd6AttqjSUsXPoOD5MdG9hUZwT5VFUuOKwitX956w/X0TVxN/ZG9U2yzAuiglztdsMFnMCSzAAVdySOp0P6z/7xn0FS/n6VSXq11QgPfCblAJL23yGReYbFwgNzUpuhNHgUmH6CLFe7aK9Ai8ad6ul5ghGO9ZAoGBAJhI7O6S2iCS/cHsyEXi3d4uSHzSJVFbZWuY0YGyDkewkHmyhbLVwbB/l5HFACvACth1UCZDZ4dgELzpARh0RlF1ltA9grtn8E056N9xgD3yR2RvT8+amH+or3mhja0HSH0e3VjsKd8e5JU1t5b3HDN4GbFo3C2fIr7y1dwQ26ZW";

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

/// The command-line references naming this test's held TLS material under
/// `secrets/`.
const TLS_ARGS: &[&str] = &[
    "--tls-certificate-secret",
    "tls.certificate",
    "--tls-private-key-secret",
    "tls.private-key",
];

/// The optional outbound trust root is an exact held DER certificate. It is
/// deliberately separate from the service listener's TLS flags: inbound
/// serving never grants outbound provider trust.
const TELEMETRY_ROOT_ARGS: &[&str] = &["--telemetry-root-certificate-secret", "telemetry.root"];

static NEXT_WORKDIR: AtomicU64 = AtomicU64::new(0);

struct Workdir {
    root: PathBuf,
}

impl Workdir {
    fn create(label: &str) -> Self {
        let root = std::env::temp_dir()
            .canonicalize()
            .expect("canonicalize test temp root");
        for _ in 0..32 {
            let nonce = NEXT_WORKDIR.fetch_add(1, Ordering::Relaxed);
            let path = root.join(format!(
                "semaprax-reference-acceptance-{label}-{}-{nonce}",
                std::process::id()
            ));
            if std::fs::create_dir(&path).is_ok() {
                for name in ["state", "outbound", "secrets", "bundle"] {
                    std::fs::create_dir(path.join(name)).unwrap();
                }
                return Self { root: path };
            }
        }
        panic!("could not allocate a unique workdir");
    }

    fn write_inputs(&self) {
        self.write_inputs_with_telemetry_origin("https://telemetry.invalid:9");
    }

    fn write_inputs_with_telemetry_origin(&self, telemetry_origin: &str) {
        self.write_inputs_with_telemetry(telemetry_origin, "semaprax-json-events");
    }

    fn write_inputs_with_telemetry(&self, telemetry_origin: &str, telemetry_adapter: &str) {
        let config = HOST_CONFIG
            .replace("https://telemetry.invalid:9", telemetry_origin)
            .replace("semaprax-json-events", telemetry_adapter);
        std::fs::write(self.root.join("service.config.json"), config).unwrap();
        std::fs::write(self.root.join("secrets").join("auth.pepper"), [1_u8; 32]).unwrap();
        std::fs::write(self.root.join("secrets").join("auth.session"), [2_u8; 32]).unwrap();
        std::fs::write(
            self.root.join("secrets").join("webhook.signing"),
            [3_u8; 32],
        )
        .unwrap();
    }

    fn write_telemetry_root_material(&self) {
        std::fs::write(
            self.root.join("secrets").join("telemetry.root"),
            decode64(TLS_ROOT),
        )
        .unwrap();
    }

    /// Hold the success-path test certificate/key under the exact reference
    /// names [`TLS_ARGS`] passes on the command line.
    fn write_tls_material(&self) {
        std::fs::write(
            self.root.join("secrets").join("tls.certificate"),
            decode64(TLS_LEAF),
        )
        .unwrap();
        std::fs::write(
            self.root.join("secrets").join("tls.private-key"),
            decode64(TLS_LEAF_KEY),
        )
        .unwrap();
    }

    /// Hold the independently generated, already-expired certificate/key
    /// under the same reference names, instead of the success-path pair.
    fn write_expired_tls_material(&self) {
        std::fs::write(
            self.root.join("secrets").join("tls.certificate"),
            decode64(EXPIRED_LEAF),
        )
        .unwrap();
        std::fs::write(
            self.root.join("secrets").join("tls.private-key"),
            decode64(EXPIRED_LEAF_KEY),
        )
        .unwrap();
    }

    fn example_project(&self) -> PathBuf {
        // The project loader rejects `.`/`..` components, so the fixture
        // path is canonicalized before the server loads it.
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("task-service-project")
            .canonicalize()
            .expect("canonicalize task-service-project fixture")
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn census(&self, name: &str) -> Vec<String> {
        let mut entries: Vec<String> = std::fs::read_dir(self.path(name))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        entries.sort();
        entries
    }
}

impl Drop for Workdir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A server child that is always killed on drop, so a failing test never
/// leaves a listener behind.
struct Server {
    child: Child,
    lines: mpsc::Receiver<String>,
    port: u16,
}

impl Server {
    fn spawn(workdir: &Workdir, extra: &[&str]) -> Self {
        Self::spawn_with_env(workdir, extra, &[])
    }

    /// Spawn with additional inherited-plus-extra environment variables. Used
    /// only to arm the debug-only crash-injection hook
    /// (`SEMAPRAX_REFERENCE_SERVICE_TEST_CRASH_AFTER_DELIVERY_JOB`, see
    /// `reference_service::mapping::crash_after_delivery_for_acceptance_test`)
    /// that proves the documented crash-safety claim against a real killed
    /// process; ordinary spawns pass an empty slice.
    fn spawn_with_env(workdir: &Workdir, extra: &[&str], envs: &[(&str, &str)]) -> Self {
        for _ in 0..5 {
            let port = free_port();
            let mut command = Command::new(SERVER);
            command
                .arg("serve")
                .arg("--project")
                .arg(workdir.example_project())
                .arg("--config")
                .arg(workdir.path("service.config.json"))
                .arg("--state-dir")
                .arg(workdir.path("state"))
                .arg("--outbound-dir")
                .arg(workdir.path("outbound"))
                .arg("--secrets-dir")
                .arg(workdir.path("secrets"))
                .arg("--bundle-dir")
                .arg(workdir.path("bundle"))
                .arg("--port")
                .arg(port.to_string())
                .args(extra)
                .envs(envs.iter().copied())
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let mut child = command.spawn().expect("spawn reference server");
            let stdout = child.stdout.take().expect("piped stdout");
            let (sender, lines) = mpsc::channel();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    if sender.send(line).is_err() {
                        break;
                    }
                }
            });
            let server = Self { child, lines, port };
            match server.wait_for_ready() {
                Ok(()) => return server,
                Err(_) => {
                    let mut server = server;
                    server.kill_and_wait();
                    // A lost port race prints a bind refusal; anything else
                    // is a real failure.
                    let mut stderr = String::new();
                    if let Some(mut pipe) = server.child.stderr.take() {
                        let _ = pipe.read_to_string(&mut stderr);
                    }
                    if !stderr.contains("cannot bind the loopback listener") {
                        panic!("server failed before ready: {stderr}");
                    }
                }
            }
        }
        panic!("could not bind a loopback port after retries");
    }

    fn wait_for_ready(&self) -> Result<(), String> {
        let deadline = Instant::now() + READY_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("server never printed ready".to_owned());
            }
            match self.lines.recv_timeout(remaining) {
                Ok(line) if line.starts_with("ready ") => return Ok(()),
                Ok(_) => continue,
                Err(_) => return Err("server output ended before ready".to_owned()),
            }
        }
    }

    fn kill_and_wait(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Wait, bounded, for the child to exit on its own (a simulated crash),
    /// rather than killing it. Panics on timeout so a hook that failed to
    /// fire is a loud test failure, not a silent hang.
    fn wait_for_exit(&mut self) -> std::process::ExitStatus {
        let deadline = Instant::now() + READY_TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait().expect("poll reference server") {
                return status;
            }
            if Instant::now() >= deadline {
                self.kill_and_wait();
                panic!("reference server did not exit on its own within the bounded wait");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.kill_and_wait();
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn http(port: u16, method: &str, target: &str, body: &str, token: Option<&str>) -> (u16, String) {
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    let mut stream = loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => break stream,
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(error) => panic!("loopback connect failed: {error}"),
        }
    };
    stream.set_read_timeout(Some(REQUEST_TIMEOUT)).unwrap();
    stream.set_write_timeout(Some(REQUEST_TIMEOUT)).unwrap();
    let mut request = format!(
        "{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(token) = token {
        request.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(body);
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let text = String::from_utf8(response).expect("server speaks UTF-8 JSON");
    let status = text
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .expect("HTTP status line");
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_owned();
    (status, body)
}

/// The same closed exchange as [`http`], but over TLS: it trusts exactly
/// `trusted_root_der` (a private/test CA) and connects to `host`, driven
/// through `semaprax::network_provider::TcpNetworkProvider` rather than
/// `rustls` directly, since this crate has no direct dependency on it. A
/// handshake failure (untrusted issuer, name mismatch, expired certificate)
/// is the hostile-case outcome some callers assert on, so it is returned
/// rather than panicking.
fn https(
    port: u16,
    host: &str,
    trusted_root_der: Vec<u8>,
    method: &str,
    target: &str,
    body: &str,
    token: Option<&str>,
) -> Result<(u16, String), ()> {
    let client_config = client_tls_config_trusting(trusted_root_der).expect("test root is valid");
    let mut provider = TcpNetworkProvider::with_tls_config(client_config);
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    let connection = loop {
        match provider.connect_tls(host, port) {
            Ok(connection) => break connection,
            // The listener may not have bound yet right after spawn; any
            // other failure (a rejected handshake) is a real, immediate
            // outcome the caller decides how to treat.
            Err(semaprax::network_provider::NetworkFailure::ConnectFailed)
                if Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return Err(()),
        }
    };
    let mut request = format!(
        "{method} {target} HTTP/1.1\r\nHost: {host}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(token) = token {
        request.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(body);
    if provider.send(connection, request.as_bytes()).is_err() {
        return Err(());
    }
    let mut response = Vec::new();
    loop {
        match provider.recv(connection, 8_192) {
            Ok(chunk) if chunk.is_empty() => break,
            Ok(chunk) => response.extend_from_slice(&chunk),
            Err(_) => break,
        }
    }
    let _ = provider.close(connection);
    let text = String::from_utf8(response).map_err(|_| ())?;
    let status = text
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or(())?;
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_owned();
    Ok((status, body))
}

/// Send one request without requiring a response: the server under test is
/// expected to crash mid-exchange (see
/// `completion_crash_after_delivery_before_commit_settles_uncertain_on_restart`),
/// so a write or read failure here is the expected outcome, not a test
/// failure. The caller separately asserts the process actually exited.
fn send_ignoring_response(port: u16, method: &str, target: &str, body: &str, token: Option<&str>) {
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    let mut stream = loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => break stream,
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(error) => panic!("loopback connect failed: {error}"),
        }
    };
    let _ = stream.set_read_timeout(Some(REQUEST_TIMEOUT));
    let _ = stream.set_write_timeout(Some(REQUEST_TIMEOUT));
    let mut request = format!(
        "{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(token) = token {
        request.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(body);
    if stream.write_all(request.as_bytes()).is_ok() {
        let mut response = Vec::new();
        let _ = stream.read_to_end(&mut response);
    }
}

fn field<'a>(body: &'a str, key: &str) -> &'a str {
    let needle = format!("\"{key}\":\"");
    let start = body
        .find(&needle)
        .unwrap_or_else(|| panic!("{key} missing in {body}"))
        + needle.len();
    let end = body[start..].find('"').unwrap();
    &body[start..start + end]
}

/// Some sandboxes deny loopback sockets outright (`EPERM` on bind). Only
/// that precise denial skips the socket acceptance test; every other
/// failure is real, and CI runs it fully.
fn loopback_denied() -> bool {
    matches!(
        std::net::TcpListener::bind(("127.0.0.1", 0)),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied
    )
}

#[test]
fn login_crud_job_restart_preserves_state_without_redispatch() {
    if loopback_denied() {
        eprintln!("skipping: sandbox denies loopback bind");
        return;
    }
    let workdir = Workdir::create("restart");
    workdir.write_inputs();

    let server = Server::spawn(&workdir, &[]);
    let port = server.port;

    let (status, body) = http(
        port,
        "POST",
        "/v1/register",
        r#"{"username":"alice","password":"correct horse 7"}"#,
        None,
    );
    assert_eq!(status, 201, "{body}");

    let (status, body) = http(
        port,
        "POST",
        "/v1/login",
        r#"{"username":"alice","password":"correct horse 7"}"#,
        None,
    );
    assert_eq!(status, 200, "{body}");
    let token = field(&body, "token").to_owned();

    let (status, body) = http(
        port,
        "POST",
        "/v1/tasks",
        r#"{"title":"write the report"}"#,
        Some(&token),
    );
    assert_eq!(status, 201, "{body}");

    let (status, body) = http(
        port,
        "PATCH",
        "/v1/tasks/1",
        r#"{"status":"done"}"#,
        Some(&token),
    );
    assert_eq!(status, 200, "{body}");

    let (status, body) = http(
        port,
        "POST",
        "/v1/jobs/enqueue",
        r#"{"key":"job-1","desc":"task-1"}"#,
        Some(&token),
    );
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "outcome"), "created");

    // No peer exists at the `.invalid` telemetry origin, so the durable
    // attempt fails closed and settles `Uncertain`; the job still
    // completes exactly once and the durable marker is committed.
    let (status, body) = http(port, "POST", "/v1/jobs/1/complete", "", Some(&token));
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "webhook"), "uncertain");
    let digest = field(&body, "state").to_owned();

    let (status, body) = http(port, "GET", "/v1/jobs/1", "", Some(&token));
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "state"), "completed");

    let outbound_before = workdir.census("outbound");
    let markers_before: Vec<_> = outbound_before
        .iter()
        .filter(|name| name.ends_with(".marker"))
        .collect();
    assert_eq!(markers_before.len(), 1, "{outbound_before:?}");
    let state_before = workdir.census("state");
    assert!(!state_before.is_empty());

    drop(server);

    // Restart from the operator-retained digest: state survives, and no
    // settled effect is duplicated.
    let server = Server::spawn(&workdir, &["--state", digest.as_str()]);
    let port = server.port;

    let (status, body) = http(port, "GET", "/v1/health", "", None);
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "state"), digest.as_str());

    let (status, body) = http(port, "GET", "/v1/tasks/1", "", Some(&token));
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "title"), "write the report");
    assert_eq!(field(&body, "status"), "done");

    let (status, body) = http(port, "GET", "/v1/jobs/1", "", Some(&token));
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "state"), "completed");
    assert_eq!(field(&body, "webhook"), "uncertain");

    let (status, _) = http(port, "POST", "/v1/jobs/1/complete", "", Some(&token));
    assert_eq!(status, 409);

    let (status, body) = http(
        port,
        "POST",
        "/v1/jobs/enqueue",
        r#"{"key":"job-1","desc":"task-1"}"#,
        Some(&token),
    );
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "outcome"), "duplicate");
    assert_eq!(field(&body, "state"), digest.as_str());

    // No new outbound file: nothing redispatched after the restart.
    assert_eq!(workdir.census("outbound"), outbound_before);
    assert_eq!(workdir.census("state"), state_before);

    // A fresh mutation still commits exactly one new snapshot.
    let (status, body) = http(
        port,
        "PATCH",
        "/v1/tasks/1",
        r#"{"status":"open"}"#,
        Some(&token),
    );
    assert_eq!(status, 200, "{body}");
    assert_ne!(field(&body, "state"), digest.as_str());
    assert_eq!(workdir.census("state").len(), state_before.len() + 1);

    // Row-level authorization survives the restart too: a second account
    // cannot read the first account's row.
    let (status, _) = http(
        port,
        "POST",
        "/v1/register",
        r#"{"username":"bob","password":"another secret 8"}"#,
        None,
    );
    assert_eq!(status, 201);
    let (status, body) = http(
        port,
        "POST",
        "/v1/login",
        r#"{"username":"bob","password":"another secret 8"}"#,
        None,
    );
    assert_eq!(status, 200, "{body}");
    let bob = field(&body, "token").to_owned();
    let (status, _) = http(port, "GET", "/v1/tasks/1", "", Some(&bob));
    assert_eq!(status, 403);

    drop(server);
    // The run bundle was written and verified at startup.
    let manifest =
        std::fs::read_to_string(workdir.path("bundle").join("bundle-manifest.json")).unwrap();
    assert!(
        manifest.contains("semaprax.reference-service.bundle.v1"),
        "{manifest}"
    );
    assert!(manifest.contains("service.config.json"), "{manifest}");
    assert!(
        manifest.contains("service-host-adapter-request.json"),
        "{manifest}"
    );
}

#[test]
fn expired_session_is_refused_by_the_checked_source_policy() {
    expired_session_policy_survives_restart(&["--session-idle-seconds", "0"]);
}

#[test]
fn coincident_session_deadlines_refuse_after_restart() {
    // idle <= absolute makes zero absolute lifetime a coincident boundary.
    // This process case proves refusal/restart; the deterministic mapping test
    // separately asserts the source-selected absolute-expired state code.
    expired_session_policy_survives_restart(&[
        "--session-idle-seconds",
        "0",
        "--session-absolute-seconds",
        "0",
    ]);
}

fn expired_session_policy_survives_restart(policy: &[&str]) {
    if loopback_denied() {
        eprintln!("skipping: sandbox denies loopback bind");
        return;
    }
    let workdir = Workdir::create("expired-session");
    workdir.write_inputs();
    // A zero-length configured host window is an intentional test policy:
    // the persisted deadline equals login's tick, and `session_is_usable`
    // rejects it on the following exchange.
    let server = Server::spawn(&workdir, policy);
    let port = server.port;
    let (status, body) = http(
        port,
        "POST",
        "/v1/register",
        r#"{"username":"alice","password":"correct horse 7"}"#,
        None,
    );
    assert_eq!(status, 201, "{body}");
    let (status, body) = http(
        port,
        "POST",
        "/v1/login",
        r#"{"username":"alice","password":"correct horse 7"}"#,
        None,
    );
    assert_eq!(status, 200, "{body}");
    let token = field(&body, "token").to_owned();
    let (status, body) = http(port, "GET", "/v1/tasks/1", "", Some(&token));
    assert_eq!(status, 401, "{body}");
    assert_eq!(field(&body, "error"), "unauthorized");
    let (status, health) = http(port, "GET", "/v1/health", "", None);
    assert_eq!(status, 200);
    let digest = field(&health, "state").to_owned();
    drop(server);
    let mut restart_args = policy.to_vec();
    restart_args.extend(["--state", digest.as_str()]);
    let restarted = Server::spawn(&workdir, &restart_args);
    let (status, body) = http(restarted.port, "GET", "/v1/tasks/1", "", Some(&token));
    assert_eq!(status, 401, "{body}");
    let (status, health) = http(restarted.port, "GET", "/v1/health", "", None);
    assert_eq!(status, 200);
    assert_eq!(
        field(&health, "state"),
        digest,
        "terminal replay must not commit again"
    );
}

#[test]
fn fixture_configuration_is_refused_without_a_runner() {
    let workdir = Workdir::create("fixture");
    workdir.write_inputs();
    // The checked-in credential-free fixture instance, verbatim.
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("task-service-project")
        .join("service.config.json");
    std::fs::copy(fixture, workdir.path("service.config.json")).unwrap();

    let output = Command::new(SERVER)
        .arg("serve")
        .arg("--project")
        .arg(workdir.example_project())
        .arg("--config")
        .arg(workdir.path("service.config.json"))
        .arg("--state-dir")
        .arg(workdir.path("state"))
        .arg("--outbound-dir")
        .arg(workdir.path("outbound"))
        .arg("--secrets-dir")
        .arg(workdir.path("secrets"))
        .arg("--bundle-dir")
        .arg(workdir.path("bundle"))
        .arg("--port")
        // The fixture refusal precedes the bind, so no listener is ever
        // opened; a fixed dummy port avoids probing for a free one.
        .arg("9")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run reference server");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("fixture-mode configuration has no host runner"),
        "{stderr}"
    );
    // Nothing was served and no bundle was written.
    assert!(workdir.census("bundle").is_empty());
}

#[test]
fn sql_configurations_refuse_before_host_binding() {
    for (adapter, diagnostic) in [
        (
            "sqlite",
            "service database adapter sqlite is unsupported; service-config.v1 admits snapshot only",
        ),
        (
            "postgresql",
            "service database adapter postgresql is unsupported; service-config.v1 admits snapshot only",
        ),
    ] {
        let workdir = Workdir::create(adapter);
        workdir.write_inputs();
        std::fs::write(
            workdir.path("service.config.json"),
            HOST_CONFIG.replace("snapshot", adapter),
        )
        .unwrap();
        let output = Command::new(SERVER)
            .arg("serve")
            .arg("--project")
            .arg(workdir.example_project())
            .arg("--config")
            .arg(workdir.path("service.config.json"))
            .arg("--state-dir")
            .arg(workdir.path("state"))
            .arg("--outbound-dir")
            .arg(workdir.path("outbound"))
            .arg("--secrets-dir")
            .arg(workdir.path("secrets"))
            .arg("--bundle-dir")
            .arg(workdir.path("bundle"))
            .arg("--port")
            .arg("9")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("run reference server");
        assert_eq!(output.status.code(), Some(2));
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(diagnostic), "{stderr}");
        for directory in ["state", "outbound", "bundle"] {
            assert!(
                workdir.census(directory).is_empty(),
                "{adapter} configuration must refuse before touching {directory}"
            );
        }
    }
}

#[test]
fn bundle_command_writes_verifies_and_rejects_tamper() {
    let workdir = Workdir::create("bundle");
    workdir.write_inputs();

    let output = Command::new(SERVER)
        .arg("bundle")
        .arg("--config")
        .arg(workdir.path("service.config.json"))
        .arg("--bundle-dir")
        .arg(workdir.path("bundle"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run bundle command");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("bundle sha256:"), "{stdout}");
    let first = stdout.trim().to_owned();

    // A byte-identical replay yields the same manifest digest.
    let output = Command::new(SERVER)
        .arg("bundle")
        .arg("--config")
        .arg(workdir.path("service.config.json"))
        .arg("--bundle-dir")
        .arg(workdir.path("bundle"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run bundle command");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), first);

    // Tampering with a bundled file breaks verification on rewrite.
    std::fs::write(
        workdir.path("bundle").join("service.config.json"),
        HOST_CONFIG.replace("snapshot", "tampered"),
    )
    .unwrap();
    let output = Command::new(SERVER)
        .arg("bundle")
        .arg("--config")
        .arg(workdir.path("service.config.json"))
        .arg("--bundle-dir")
        .arg(workdir.path("bundle"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run bundle command");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
}

/// The debug-only crash-injection hook this harness arms via
/// `SEMAPRAX_REFERENCE_SERVICE_TEST_CRASH_AFTER_DELIVERY_JOB` (see
/// `reference_service::mapping::crash_after_delivery_for_acceptance_test`).
const CRASH_ENV: &str = "SEMAPRAX_REFERENCE_SERVICE_TEST_CRASH_AFTER_DELIVERY_JOB";
/// The exact process exit code the hook uses, so a real bug elsewhere that
/// happens to kill the process cannot be confused with the hook firing.
const CRASH_EXIT_CODE: i32 = 91;

/// `complete_job`'s durable webhook-delivery attempt precedes its
/// `ServiceState` commit (`mapping.rs` module docs, and
/// `docs/REFERENCE-SERVICE-HOST-V1.md`'s "Persistence and restart" claim):
/// a crash strictly between the two must leave a pending job whose durable
/// delivery marker already exists, so a retry settles `Uncertain` instead of
/// redispatching, and the job still completes exactly once. This proves
/// that exact interleave against a real killed and restarted process,
/// distinct from `login_crud_job_restart_preserves_state_without_redispatch`
/// above, which only ever observes a delivery attempt that fails closed
/// over the network (no crash involved) and a clean restart afterward.
#[test]
fn completion_crash_after_delivery_before_commit_settles_uncertain_on_restart() {
    if loopback_denied() {
        eprintln!("skipping: sandbox denies loopback bind");
        return;
    }
    let workdir = Workdir::create("crash-interleave");
    workdir.write_inputs();

    // First run: register, log in, and enqueue exactly one job, so there is
    // one pending job to complete afterward. Clean shutdown (no crash hook).
    let server = Server::spawn(&workdir, &[]);
    let port = server.port;

    let (status, body) = http(
        port,
        "POST",
        "/v1/register",
        r#"{"username":"alice","password":"correct horse 7"}"#,
        None,
    );
    assert_eq!(status, 201, "{body}");
    let (status, body) = http(
        port,
        "POST",
        "/v1/login",
        r#"{"username":"alice","password":"correct horse 7"}"#,
        None,
    );
    assert_eq!(status, 200, "{body}");
    let token = field(&body, "token").to_owned();
    let (status, body) = http(
        port,
        "POST",
        "/v1/jobs/enqueue",
        r#"{"key":"job-1","desc":"task-1"}"#,
        Some(&token),
    );
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "outcome"), "created");
    let digest_before_completion = field(&body, "state").to_owned();

    drop(server);
    let outbound_before_crash = workdir.census("outbound");
    assert!(
        outbound_before_crash
            .iter()
            .all(|name| !name.ends_with(".marker")),
        "no delivery has been attempted yet: {outbound_before_crash:?}"
    );

    // Second run: resume from the pre-completion digest with the crash hook
    // armed for job 1. The completion request's durable webhook-delivery
    // attempt commits its marker and settles (`uncertain`, since no peer
    // exists at the `.invalid` telemetry origin) before the injected crash
    // exits the whole process -- strictly before the `ServiceState` commit
    // that would mark the job completed.
    let mut crashing = Server::spawn_with_env(
        &workdir,
        &["--state", digest_before_completion.as_str()],
        &[(CRASH_ENV, "1")],
    );
    let crash_port = crashing.port;
    send_ignoring_response(crash_port, "POST", "/v1/jobs/1/complete", "", Some(&token));
    let status = crashing.wait_for_exit();
    assert_eq!(
        status.code(),
        Some(CRASH_EXIT_CODE),
        "the crash hook must have fired: {status:?}"
    );

    let outbound_after_crash = workdir.census("outbound");
    let markers_after_crash: Vec<_> = outbound_after_crash
        .iter()
        .filter(|name| name.ends_with(".marker"))
        .collect();
    assert_eq!(markers_after_crash.len(), 1, "{outbound_after_crash:?}");
    let state_before_retry = workdir.census("state");

    // Third run: restart from the SAME pre-completion digest -- the crash
    // means the `ServiceState` commit never landed -- with no crash hook
    // armed. The job is still pending and unsettled in state.
    let server = Server::spawn(&workdir, &["--state", digest_before_completion.as_str()]);
    let port = server.port;
    let (status, body) = http(port, "GET", "/v1/jobs/1", "", Some(&token));
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "state"), "pending");
    assert_eq!(field(&body, "webhook"), "none");

    // Retrying completion now succeeds: the durable delivery marker already
    // exists, so this reconciles/replays instead of redispatching to the
    // provider, and the `ServiceState` commit -- which never landed before
    // -- now does.
    let (status, body) = http(port, "POST", "/v1/jobs/1/complete", "", Some(&token));
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "webhook"), "uncertain");
    let digest_after_completion = field(&body, "state").to_owned();
    assert_ne!(digest_after_completion, digest_before_completion);

    let (status, body) = http(port, "GET", "/v1/jobs/1", "", Some(&token));
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "state"), "completed");
    assert_eq!(field(&body, "webhook"), "uncertain");

    // No second dispatch: the outbound census is unchanged from right after
    // the crash (the same one marker; the replay wrote nothing new), while
    // the state census gained exactly the one new completion snapshot.
    assert_eq!(workdir.census("outbound"), outbound_after_crash);
    assert_eq!(workdir.census("state").len(), state_before_retry.len() + 1);
}

/// The same login/CRUD/job/restart flow as
/// `login_crud_job_restart_preserves_state_without_redispatch`, run entirely
/// over TLS with a test CA: optional TLS serving, configured only through
/// operator-held certificate/key material under `--secrets-dir`, carries the
/// whole exchange end to end, including physical persistence across a real
/// kill-and-restart.
#[test]
fn tls_login_crud_job_restart_preserves_state_without_redispatch() {
    if loopback_denied() {
        eprintln!("skipping: sandbox denies loopback bind");
        return;
    }
    let workdir = Workdir::create("tls-restart");
    workdir.write_inputs();
    workdir.write_tls_material();
    let root = decode64(TLS_ROOT);

    let server = Server::spawn(&workdir, TLS_ARGS);
    let port = server.port;

    let (status, body) = https(
        port,
        "localhost",
        root.clone(),
        "POST",
        "/v1/register",
        r#"{"username":"alice","password":"correct horse 7"}"#,
        None,
    )
    .expect("register over TLS");
    assert_eq!(status, 201, "{body}");

    let (status, body) = https(
        port,
        "localhost",
        root.clone(),
        "POST",
        "/v1/login",
        r#"{"username":"alice","password":"correct horse 7"}"#,
        None,
    )
    .expect("login over TLS");
    assert_eq!(status, 200, "{body}");
    let token = field(&body, "token").to_owned();

    let (status, body) = https(
        port,
        "localhost",
        root.clone(),
        "POST",
        "/v1/tasks",
        r#"{"title":"write the report"}"#,
        Some(&token),
    )
    .expect("create task over TLS");
    assert_eq!(status, 201, "{body}");

    let (status, body) = https(
        port,
        "localhost",
        root.clone(),
        "PATCH",
        "/v1/tasks/1",
        r#"{"status":"done"}"#,
        Some(&token),
    )
    .expect("patch task over TLS");
    assert_eq!(status, 200, "{body}");

    let (status, body) = https(
        port,
        "localhost",
        root.clone(),
        "POST",
        "/v1/jobs/enqueue",
        r#"{"key":"job-1","desc":"task-1"}"#,
        Some(&token),
    )
    .expect("enqueue job over TLS");
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "outcome"), "created");

    let (status, body) = https(
        port,
        "localhost",
        root.clone(),
        "POST",
        "/v1/jobs/1/complete",
        "",
        Some(&token),
    )
    .expect("complete job over TLS");
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "webhook"), "uncertain");
    let digest = field(&body, "state").to_owned();

    let (status, body) = https(
        port,
        "localhost",
        root.clone(),
        "GET",
        "/v1/jobs/1",
        "",
        Some(&token),
    )
    .expect("read job over TLS");
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "state"), "completed");

    let state_before = workdir.census("state");
    assert!(!state_before.is_empty());

    drop(server);

    // Restart still requires the same TLS flags; nothing about persistence
    // or restart depends on which transport served the earlier requests.
    let mut restart_args = vec!["--state", digest.as_str()];
    restart_args.extend_from_slice(TLS_ARGS);
    let server = Server::spawn(&workdir, &restart_args);
    let port = server.port;

    let (status, body) = https(
        port,
        "localhost",
        root.clone(),
        "GET",
        "/v1/health",
        "",
        None,
    )
    .expect("health over TLS after restart");
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "state"), digest.as_str());

    let (status, body) = https(
        port,
        "localhost",
        root.clone(),
        "GET",
        "/v1/tasks/1",
        "",
        Some(&token),
    )
    .expect("read task over TLS after restart");
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "title"), "write the report");
    assert_eq!(field(&body, "status"), "done");

    // Duplicate enqueue after restart is still recognized: no redispatch.
    let (status, body) = https(
        port,
        "localhost",
        root.clone(),
        "POST",
        "/v1/jobs/enqueue",
        r#"{"key":"job-1","desc":"task-1"}"#,
        Some(&token),
    )
    .expect("duplicate enqueue over TLS after restart");
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "outcome"), "duplicate");
    assert_eq!(field(&body, "state"), digest.as_str());
    assert_eq!(workdir.census("state"), state_before);

    drop(server);
}

/// TLS requested on the command line (both `--tls-*-secret` flags) without
/// any held certificate/key material under `--secrets-dir` is refused
/// before any listener binds and before the run bundle is written:
/// configuration intent never mints authority.
#[test]
fn tls_requested_without_held_material_is_refused() {
    let workdir = Workdir::create("tls-unheld");
    workdir.write_inputs();
    // Deliberately do not call `write_tls_material`: the two references are
    // named on the command line but nothing is held under either name.

    let output = Command::new(SERVER)
        .arg("serve")
        .arg("--project")
        .arg(workdir.example_project())
        .arg("--config")
        .arg(workdir.path("service.config.json"))
        .arg("--state-dir")
        .arg(workdir.path("state"))
        .arg("--outbound-dir")
        .arg(workdir.path("outbound"))
        .arg("--secrets-dir")
        .arg(workdir.path("secrets"))
        .arg("--bundle-dir")
        .arg(workdir.path("bundle"))
        .arg("--port")
        // The refusal precedes the bind, so no listener is ever opened; a
        // fixed dummy port avoids probing for a free one.
        .arg("9")
        .args(TLS_ARGS)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run reference server");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("cannot resolve the held TLS certificate or private key"),
        "{stderr}"
    );
    assert!(workdir.census("bundle").is_empty());
}

/// A client that does not trust the exact certificate authority that issued
/// the server's certificate is refused (`NetworkFailure::TlsFailed` inside
/// `connect_tls`), even though the certificate names the exact host being
/// dialed.
#[test]
fn tls_server_rejects_a_client_that_does_not_trust_its_issuer() {
    if loopback_denied() {
        eprintln!("skipping: sandbox denies loopback bind");
        return;
    }
    let workdir = Workdir::create("tls-wrong-issuer");
    workdir.write_inputs();
    workdir.write_tls_material();
    let server = Server::spawn(&workdir, TLS_ARGS);
    let port = server.port;

    // Trust a real, but unrelated, private test CA -- not the one that
    // issued this server's certificate.
    let unrelated_root = decode64(EXPIRED_CA_ROOT);
    let result = https(
        port,
        "localhost",
        unrelated_root,
        "GET",
        "/v1/health",
        "",
        None,
    );
    assert!(
        result.is_err(),
        "a certificate no installed root vouches for must be refused: {result:?}"
    );
}

/// A client that trusts the right issuer, but dials the one name the
/// certificate's SAN does not cover, is refused.
#[test]
fn tls_server_rejects_a_hostname_the_certificate_does_not_name() {
    if loopback_denied() {
        eprintln!("skipping: sandbox denies loopback bind");
        return;
    }
    let workdir = Workdir::create("tls-wrong-name");
    workdir.write_inputs();
    workdir.write_tls_material();
    let server = Server::spawn(&workdir, TLS_ARGS);
    let port = server.port;

    // The leaf's SAN covers only "localhost"; the server also listens on
    // 127.0.0.1, the loopback address, so the endpoint itself is reachable.
    let root = decode64(TLS_ROOT);
    let result = https(port, "127.0.0.1", root, "GET", "/v1/health", "", None);
    assert!(
        result.is_err(),
        "a trusted chain is not enough; the name must match too: {result:?}"
    );
}

/// A client that trusts the right issuer and the right name, but whose
/// certificate's whole validity window is in the past, is refused.
#[test]
fn tls_server_rejects_an_expired_certificate() {
    if loopback_denied() {
        eprintln!("skipping: sandbox denies loopback bind");
        return;
    }
    let workdir = Workdir::create("tls-expired");
    workdir.write_inputs();
    workdir.write_expired_tls_material();
    let server = Server::spawn(&workdir, TLS_ARGS);
    let port = server.port;

    let root = decode64(EXPIRED_CA_ROOT);
    let result = https(port, "localhost", root, "GET", "/v1/health", "", None);
    assert!(
        result.is_err(),
        "an expired certificate must be refused even under its own trusted issuer: {result:?}"
    );
}

/// A TLS-only listener never falls back to plaintext framing: a plaintext
/// client's bytes fail the TLS handshake, so the peer gets at most a raw
/// TLS alert record and never an HTTP response.
#[test]
fn tls_listener_refuses_a_plaintext_client() {
    if loopback_denied() {
        eprintln!("skipping: sandbox denies loopback bind");
        return;
    }
    let workdir = Workdir::create("tls-plaintext-refused");
    workdir.write_inputs();
    workdir.write_tls_material();
    let server = Server::spawn(&workdir, TLS_ARGS);
    let port = server.port;

    let deadline = Instant::now() + REQUEST_TIMEOUT;
    let mut stream = loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => break stream,
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(error) => panic!("loopback connect failed: {error}"),
        }
    };
    let _ = stream.set_read_timeout(Some(REQUEST_TIMEOUT));
    let _ = stream
        .write_all(b"GET /v1/health HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\n\r\n");
    let mut response = Vec::new();
    let _ = stream.read_to_end(&mut response);
    assert!(
        !response.starts_with(b"HTTP/"),
        "a plaintext client must never get an HTTP response from a TLS-only listener \
         (a raw TLS alert record, or nothing, is fine): {response:?}"
    );
}

#[test]
fn packaged_development_service_runs_from_an_independent_workspace() {
    if loopback_denied() {
        eprintln!("skipping: sandbox denies loopback bind");
        return;
    }
    let inputs = Workdir::create("installed-development");
    inputs.write_inputs();
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let executable = std::env::var("SEMAPRAX_TEST_REFERENCE_SERVICE_STATIC_EXECUTABLE")
        .unwrap_or_else(|_| SERVER.to_owned());
    let output = Command::new("python3")
        .arg(repository.join("scripts/tests/reference_service_installed_development.py"))
        .arg("--packager")
        .arg(repository.join("scripts/package-reference-service.py"))
        .arg("--checker")
        .arg(&executable)
        .arg("--executable")
        .arg(&executable)
        .arg("--project")
        .arg(inputs.example_project())
        .arg("--config")
        .arg(inputs.path("service.config.json"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run packaged installed-development journey");
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "packaged reference-service installed-development journey passed\n"
    );
}

/// This gate needs an operator-supplied static Linux executable and a local
/// Podman runtime. It imports the exact OCI layout and exercises mounted
/// physical adapters; keep it opt-in so ordinary macOS/Linux compiler gates do
/// not infer that every host has a container runtime.
#[test]
#[ignore = "requires SEMAPRAX_REFERENCE_SERVICE_OCI_EXECUTABLE and local Podman on Linux"]
fn packaged_oci_service_runs_with_physical_adapters() {
    let executable = std::env::var_os("SEMAPRAX_REFERENCE_SERVICE_OCI_EXECUTABLE")
        .map(PathBuf::from)
        .expect("set SEMAPRAX_REFERENCE_SERVICE_OCI_EXECUTABLE to a static Linux service binary");
    let podman = std::env::var_os("SEMAPRAX_REFERENCE_SERVICE_PODMAN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("podman"));
    let inputs = Workdir::create("oci-runtime");
    inputs.write_inputs();
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let output = Command::new("python3")
        .arg(repository.join("scripts/tests/reference_service_oci_runtime.py"))
        .arg("--packager")
        .arg(repository.join("scripts/package-reference-service.py"))
        .arg("--checker")
        .arg(&executable)
        .arg("--executable")
        .arg(&executable)
        .arg("--project")
        .arg(inputs.example_project())
        .arg("--config")
        .arg(inputs.path("service.config.json"))
        .arg("--podman")
        .arg(podman)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run packaged OCI runtime journey");
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "packaged reference-service OCI runtime journey passed\n"
    );
}

#[test]
fn package_preflight_checks_service_without_runtime_grants() {
    let workdir = Workdir::create("package-preflight");
    std::fs::write(workdir.path("service.config.json"), HOST_CONFIG).unwrap();
    let check = |project: PathBuf| {
        Command::new(SERVER)
            .arg("check-package")
            .arg("--project")
            .arg(project)
            .arg("--config")
            .arg(workdir.path("service.config.json"))
            .output()
            .unwrap()
    };
    let accepted = check(workdir.example_project());
    assert!(accepted.status.success(), "{:?}", accepted);
    assert_eq!(
        String::from_utf8(accepted.stdout).unwrap(),
        "checked reference-service package inputs\n"
    );
    for directory in ["state", "outbound", "secrets", "bundle"] {
        assert_eq!(
            std::fs::read_dir(workdir.path(directory)).unwrap().count(),
            0
        );
    }
    // Host intent alone does not authorize an invalid or absent project.
    assert_eq!(
        check(workdir.path("missing-project")).status.code(),
        Some(2)
    );
    let fixture = workdir.example_project().join("service.config.json");
    std::fs::copy(fixture, workdir.path("service.config.json")).unwrap();
    let refused = check(workdir.example_project());
    assert_eq!(refused.status.code(), Some(2));
    assert!(String::from_utf8(refused.stderr)
        .unwrap()
        .contains("package needs valid host-mode configuration"));
}
