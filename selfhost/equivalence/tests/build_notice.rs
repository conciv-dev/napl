use build_notice::notice;

#[test]
fn notice_is_the_exact_deprecation_message() {
    assert_eq!(
        notice(),
        "napl build is deprecated. Generation now works directly from prompts — the coding agent writes source, and the IR is derived afterwards. Run \"napl gen <target>\" instead."
    );
}
