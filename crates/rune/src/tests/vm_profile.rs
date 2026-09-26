prelude!();

use crate::runtime::profile;

#[test]
fn profiling_counts_each_function_by_its_own_instructions_and_calls_and_restarts_empty() {
    profile::set_profiling(true);
    let out: i64 = rune! {
        fn step(n) { n + 1 }
        let t = 0;
        for i in 0..10 {
            t = step(t);
        }
        t
    };
    assert_eq!(out, 10);
    let costs = profile::snapshot();
    let step = costs
        .iter()
        .find(|cost| cost.path.ends_with("step"))
        .unwrap_or_else(|| panic!("step was counted: {costs:?}"));
    assert_eq!(step.calls, 10);
    assert!(step.instructions >= 10, "{step:?}");
    let total: u64 = costs.iter().map(|cost| cost.instructions).sum();
    assert!(
        total > step.instructions,
        "the caller's loop counts too: {costs:?}"
    );

    // One test rather than two: the switch is process-wide, and a second test
    // turning it off would cut this one's counting short.
    profile::set_profiling(true);
    assert!(
        profile::snapshot().is_empty(),
        "turning it on clears the counts"
    );
    profile::set_profiling(false);
}
