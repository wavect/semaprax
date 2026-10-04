fn main() {
    assert_eq!(ri06_regex_owner::run(), Ok(41));
    assert_eq!(ri06_url_owner::run(), Ok(41));
    assert!(ri06_regex_owner::projected_borrow_matches_target());
    assert!(ri06_url_owner::projected_borrow_matches_target());
    assert_eq!(ri06_regex_owner::spx_result_owner_adapter_copies(), 0);
    assert_eq!(ri06_regex_owner::live_owner_count(), 0);
    assert_eq!(ri06_regex_owner::live_string_count(), 0);
    assert_eq!(ri06_url_owner::live_string_count(), 0);
    println!("ri13-m1-regex-url-ok");
}
