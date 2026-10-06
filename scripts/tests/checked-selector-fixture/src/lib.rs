//! Cases with known listing, ignore and execution behaviour for the checked
//! test-selector controller in `scripts/ci-msrv.py`.

#[cfg(test)]
mod family {
    #[test]
    fn first() {}

    #[test]
    fn second() {
        println!("test family::first ... ok");
    }

    #[test]
    #[ignore = "deliberately ignored family member"]
    fn ignored_member() {}

    #[test]
    #[should_panic(expected = "intended")]
    fn panics() {
        panic!("intended");
    }
}

#[cfg(test)]
mod family_lookalike {
    #[test]
    fn outside_the_family() {}
}

#[cfg(test)]
mod escape {
    #[test]
    fn inside() {}
}

#[cfg(test)]
mod nested {
    mod escape {
        #[test]
        fn outside() {}
    }
}

#[cfg(test)]
mod only_ignored {
    #[test]
    #[ignore]
    fn alone() {}
}

#[cfg(test)]
mod gated {
    #[test]
    fn fails_on_request() {
        assert!(std::env::var_os("CHECKED_FIXTURE_FAIL").is_none());
    }

    #[test]
    fn aborts_on_request() {
        if std::env::var_os("CHECKED_FIXTURE_ABORT").is_some() {
            std::process::abort();
        }
    }
}
