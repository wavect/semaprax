use super::*;

#[test]
fn source_index_runs_negotiates_answers_idles_restarts_and_exits() {
    let fx = fixture();
    let m = mgr(|_| {});
    let h = m.prepare(PROJECT, source_index(&fx)).unwrap();
    assert_eq!(h.state(), AdapterState::Prepared);
    assert_eq!(
        h.pid(),
        None,
        "lazy start: nothing runs before the first invoke"
    );
    assert_eq!(h.isolation_mode(), IsolationMode::Subprocess);

    let cancel = CancelToken::new();
    let out = h.invoke(
        &search("inv-1", "alpha_symbol"),
        InvocationClass::SafeRead,
        &cancel,
    );
    let Outcome::Completed(env) = out else {
        panic!("{out:?}")
    };
    assert!(env.payload.unwrap().to_string().contains("alpha_symbol"));
    assert_eq!(h.state(), AdapterState::Active);
    let pid = h.pid().unwrap();
    assert!(matches!(
        h.invoke(
            &search("inv-2", "alpha"),
            InvocationClass::SafeRead,
            &cancel
        ),
        Outcome::Completed(_)
    ));
    assert_eq!(h.pid(), Some(pid), "one process serves repeated calls");

    // Injected clock: not idle yet, then idle.
    assert!(!h.reap_idle(h.last_used()));
    assert_eq!(
        m.reap_idle(h.last_used() + Duration::from_secs(61)),
        vec![(PROJECT.to_string(), h.provider_id().to_string())]
    );
    assert_eq!(h.state(), AdapterState::Prepared);
    assert_gone(pid);
    let out = h.invoke(
        &search("inv-3", "alpha"),
        InvocationClass::SafeRead,
        &cancel,
    );
    assert!(
        matches!(out, Outcome::Completed(_)),
        "lazy restart after idle: {out:?}"
    );
    assert_ne!(h.pid(), Some(pid));

    let pid = h.pid().unwrap();
    h.shutdown();
    assert_eq!(h.state(), AdapterState::Closed);
    assert_gone(pid);
    assert!(
        matches!(h.invoke(&search("inv-4", "alpha"), InvocationClass::SafeRead, &cancel), Outcome::Refused(d) if d.code == "SPX-HPC021")
    );
}

#[test]
fn adapters_are_isolated_per_project_and_requests_are_bound_to_their_project() {
    let (fa, fb) = (fixture(), fixture());
    let m = mgr(|_| {});
    let a = m.prepare("p1", source_index(&fa)).unwrap();
    let b = m.prepare("p2", source_index(&fb)).unwrap();
    assert!(!std::sync::Arc::ptr_eq(&a, &b));
    assert!(std::sync::Arc::ptr_eq(
        &a,
        &m.prepare("p1", source_index(&fa)).unwrap()
    ));
    let c = CancelToken::new();
    assert!(matches!(
        a.invoke(&search("i1", "alpha"), InvocationClass::SafeRead, &c),
        Outcome::Completed(_)
    ));
    // A p1-bound request is refused by p2's handle, which stays unstarted.
    assert!(
        matches!(b.invoke(&search("i2", "alpha"), InvocationClass::SafeRead, &c), Outcome::Refused(d) if d.code == "SPX-HPC020")
    );
    assert_eq!(b.pid(), None);
    let mut req = search("i3", "alpha");
    req.project.id = "p2".into();
    assert!(matches!(
        b.invoke(&req, InvocationClass::SafeRead, &c),
        Outcome::Completed(_)
    ));
    assert_ne!(a.pid(), b.pid());
}

#[test]
fn undeclared_operation_is_refused_before_it_is_sent() {
    let fx = fixture();
    let m = mgr(|_| {});
    let h = m.prepare(PROJECT, hostile(&fx, "")).unwrap();
    // hostile does not declare context ops accepted for command.wrap
    let req = request(
        CapabilityKind::CommandView,
        "wrap",
        json!({"form": "wrapper", "argv": ["ls"]}),
        "i1",
    );
    assert!(
        matches!(h.invoke(&req, InvocationClass::SafeRead, &CancelToken::new()), Outcome::Refused(d) if d.code == "SPX-HPC020")
    );
}

#[test]
fn changed_entry_or_wrong_grant_refuses_launch() {
    let fx = fixture();
    let dir = fx.root.join("adapter");
    std::fs::create_dir_all(&dir).unwrap();
    for f in ["adapter.py", "harness-provider.json"] {
        std::fs::copy(examples().join("hostile-python").join(f), dir.join(f)).unwrap();
    }
    let d = descriptor_from(&dir, |_| {});
    let m = mgr(|_| {});
    let h = m
        .prepare(
            PROJECT,
            spec_in(&dir, d.clone(), &fx, &[], IsolationRequest::None),
        )
        .unwrap();
    std::fs::write(
        dir.join("adapter.py"),
        "#!/usr/bin/env python3\nprint('tampered')\n",
    )
    .unwrap();
    let out = h.invoke(
        &decide("i1"),
        InvocationClass::SafeRead,
        &CancelToken::new(),
    );
    assert!(
        matches!(&out, Outcome::Refused(d) if d.code == "SPX-HPC002"),
        "{out:?}"
    );
    assert_eq!(h.pid(), None, "nothing was started");
    // Preparing again against the tampered bytes is refused up front.
    let m2 = mgr(|_| {});
    assert_eq!(
        m2.prepare(
            PROJECT,
            spec_in(&dir, d.clone(), &fx, &[], IsolationRequest::None)
        )
        .err()
        .map(|e| e.code),
        None,
        "grant built from current bytes is fine"
    );

    // Grant for a different descriptor digest.
    let mut s = hostile(&fx, "");
    s.grant = Grant::issue(
        s.descriptor.provider_id.clone(),
        "sha256:bad".into(),
        s.grant.entry_digest().map(String::from),
        None,
        GrantedPermissions::default(),
    );
    assert_eq!(
        mgr(|_| {}).prepare(PROJECT, s).err().unwrap().code,
        "SPX-HPC001"
    );
    // Reserved env keys cannot be forwarded.
    let mut s = hostile(&fx, "");
    s.forward_env
        .insert("SEMAPRAX_HARNESS_UPSTREAM".into(), "/bin/sh".into());
    assert_eq!(
        mgr(|_| {}).prepare(PROJECT, s).err().unwrap().code,
        "SPX-HPC001"
    );
    // Upstream digest without an upstream executable (or vice versa) is refused.
    let mut s = hostile(&fx, "");
    s.upstream_executable = Some("/bin/sh".into());
    assert_eq!(
        mgr(|_| {}).prepare(PROJECT, s).err().unwrap().code,
        "SPX-HPC002"
    );
}

#[test]
fn host_budget_is_separate_and_exhausts() {
    let fx = fixture();
    let m = mgr(|c| {
        c.budget = HostBudget {
            max_jobs: 2,
            ..HostBudget::default()
        }
    });
    let h = m.prepare(PROJECT, source_index(&fx)).unwrap();
    let c = CancelToken::new();
    for i in 0..2 {
        assert!(matches!(
            h.invoke(
                &search(&format!("i{i}"), "alpha"),
                InvocationClass::SafeRead,
                &c
            ),
            Outcome::Completed(_)
        ));
    }
    assert!(
        matches!(h.invoke(&search("i9", "alpha"), InvocationClass::SafeRead, &c), Outcome::Refused(d) if d.code == "SPX-HPC022")
    );
    let used = m.budget_used();
    assert_eq!(used.jobs, 2);
    assert!(used.output_bytes > 0);
}

#[test]
fn queue_is_bounded() {
    let fx = fixture();
    let m = mgr(|c| c.max_queue = 0);
    let h = m.prepare(PROJECT, hostile(&fx, "ignore_cancel")).unwrap();
    let h2 = h.clone();
    let t = std::thread::spawn(move || {
        let mut r = decide("slow");
        r.deadline_ms = 700;
        h2.invoke(&r, InvocationClass::SafeRead, &CancelToken::new())
    });
    let (_, grandchild) = wait_pids(&fx);
    let out = h.invoke(
        &decide("second"),
        InvocationClass::SafeRead,
        &CancelToken::new(),
    );
    assert!(matches!(out, Outcome::Refused(d) if d.code == "SPX-HPC017"));
    assert!(matches!(t.join().unwrap(), Outcome::Unavailable { .. }));
    assert_gone(grandchild);
}

#[test]
fn hang_on_initialize_hits_the_handshake_timeout_and_is_killed() {
    let fx = fixture();
    let m = mgr(|_| {});
    let h = m
        .prepare(PROJECT, hostile(&fx, "hang_on_initialize"))
        .unwrap();
    let t = Instant::now();
    let out = h.invoke(
        &decide("i1"),
        InvocationClass::SafeRead,
        &CancelToken::new(),
    );
    assert!(t.elapsed() < Duration::from_secs(4));
    let Outcome::Unavailable {
        reason,
        request_sent: false,
        fallback_allowed: true,
    } = out
    else {
        panic!("{out:?}")
    };
    assert_eq!(reason.code, "SPX-HPC005");
    assert_eq!(h.pid(), None);
    assert!(matches!(h.state(), AdapterState::Unavailable(d) if d.code == "SPX-HPC005"));
}

#[test]
fn ignore_cancel_is_killed_with_its_grandchild_after_cancel() {
    let fx = fixture();
    let m = mgr(|c| c.cancel_grace_ms = 100);
    let h = m.prepare(PROJECT, hostile(&fx, "ignore_cancel")).unwrap();
    let cancel = CancelToken::new();
    let c2 = cancel.clone();
    let fxr = fx.root.clone();
    let canceller = std::thread::spawn(move || {
        let end = Instant::now() + Duration::from_secs(10);
        while !fxr.join("pids").exists() && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(20));
        }
        c2.cancel();
    });
    let out = h.invoke(&decide("i1"), InvocationClass::SafeRead, &cancel);
    canceller.join().unwrap();
    assert_eq!(out, Outcome::Cancelled);
    let (adapter, grandchild) = wait_pids(&fx);
    assert_gone(adapter);
    assert_gone(grandchild);
    assert_eq!(h.pid(), None);
    assert_eq!(h.state(), AdapterState::Prepared, "cancel is not a crash");
}

#[test]
fn deadline_kills_group_and_respects_the_retry_boundary() {
    for (class, expect_uncertain) in [
        (InvocationClass::SafeRead, false),
        (InvocationClass::SideEffecting, true),
    ] {
        let fx = fixture();
        let m = mgr(|_| {});
        let h = m.prepare(PROJECT, hostile(&fx, "ignore_cancel")).unwrap();
        let mut r = decide("i1");
        r.deadline_ms = 400;
        let out = h.invoke(&r, class, &CancelToken::new());
        let (adapter, grandchild) = wait_pids(&fx);
        assert_gone(adapter);
        assert_gone(grandchild);
        match out {
            Outcome::Uncertain(d) if expect_uncertain => assert_eq!(d.code, "SPX-HPC008"),
            Outcome::Unavailable {
                reason,
                request_sent: true,
                fallback_allowed: true,
            } if !expect_uncertain => assert_eq!(reason.code, "SPX-HPC008"),
            o => panic!("{class:?}: {o:?}"),
        }
    }
}

#[test]
fn crash_falls_back_only_when_safe_and_the_breaker_quarantines() {
    let fx = fixture();
    let m = mgr(|c| c.crash_threshold = 3);
    let h = m.prepare(PROJECT, hostile(&fx, "crash_on_invoke")).unwrap();
    let c = CancelToken::new();
    let out = h.invoke(&decide("i1"), InvocationClass::SafeRead, &c);
    assert!(
        matches!(&out, Outcome::Unavailable { request_sent: true, fallback_allowed: true, reason } if reason.code == "SPX-HPC007"),
        "{out:?}"
    );
    let out = h.invoke(&decide("i2"), InvocationClass::SideEffecting, &c);
    assert!(
        matches!(&out, Outcome::Uncertain(d) if d.code == "SPX-HPC007"),
        "{out:?}"
    );
    assert_eq!(h.consecutive_crashes(), 2);
    assert!(matches!(h.state(), AdapterState::Unavailable(_)));
    // Decision class may fall back like a safe read; third crash opens the breaker.
    assert!(matches!(
        h.invoke(&decide("i3"), InvocationClass::Decision, &c),
        Outcome::Unavailable {
            fallback_allowed: true,
            ..
        }
    ));
    assert!(matches!(h.state(), AdapterState::Quarantined(d) if d.code == "SPX-HPC016"));
    assert!(
        matches!(h.invoke(&decide("i4"), InvocationClass::SafeRead, &c), Outcome::Quarantined(d) if d.code == "SPX-HPC016")
    );
}

#[test]
fn stderr_flood_never_blocks_and_is_bounded() {
    let fx = fixture();
    let m = mgr(|_| {});
    let h = m.prepare(PROJECT, hostile(&fx, "stderr_flood")).unwrap();
    let t = Instant::now();
    let out = h.invoke(
        &decide("i1"),
        InvocationClass::SafeRead,
        &CancelToken::new(),
    );
    assert!(matches!(out, Outcome::Completed(_)), "{out:?}");
    assert!(t.elapsed() < Duration::from_secs(10));
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        let (tail, dropped) = h.stderr_tail();
        assert!(tail.len() <= 64 * 1024);
        if dropped > 0 || Instant::now() > end {
            assert!(dropped > 0, "the ring must report dropped bytes");
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
