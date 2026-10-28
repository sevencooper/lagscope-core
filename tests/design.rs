use lagscope_core::design::TOKENS_JSON;
use lagscope_core::stream::jitter;

#[test]
fn tokens_are_exported() {
    assert!(TOKENS_JSON.contains("\"mint\": \"#5BE7C4\""));
}

#[test]
fn jitter_is_the_average_change() {
    assert_eq!(jitter(&[]), 0.0);
    assert_eq!(jitter(&[20.0]), 0.0);
    // |30-10| + |10-20| = 30 over 2 changes.
    assert_eq!(jitter(&[10.0, 30.0, 20.0]), 15.0);
}
