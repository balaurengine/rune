//! Balaur fork: the default hasher must be fixed and portable.
//!
//! `ahash` was neither. Its `BuildHasherDefault` seed comes from `getrandom`
//! once per process whenever its `runtime-rng` feature is on, which any crate
//! in the dependency graph can turn on for everyone, and its AES and software
//! paths produce different digests, so two platforms disagree even at the same
//! seed. Rune leaks that order to scripts through `Object::iter`, `keys` and
//! `values`, and the hash itself through `std::ops::hash`.
//!
//! The replacement is `XxHash64`, which `rune-core` already uses for `Hash`.

use core::hash::{BuildHasher, Hasher};

use rune_alloc::hashbrown::map::DefaultHashBuilder;
use rune_alloc::HashMap;

fn hash_of(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHashBuilder::default().build_hasher();
    hasher.write(bytes);
    hasher.finish()
}

/// XxHash64's published test vectors, at seed 0. Passing these is what makes
/// "the same on every target" a checkable claim rather than an intention.
#[test]
fn matches_the_xxhash64_specification() {
    assert_eq!(hash_of(b""), 0xef46_db37_51d8_e999);
    assert_eq!(hash_of(b"a"), 0xd24e_c4f1_a98c_6e5b);
}

#[test]
fn two_builders_agree() {
    assert_eq!(hash_of(b"update"), hash_of(b"update"));
    assert_ne!(hash_of(b"update"), hash_of(b"fixed_update"));
}

/// The property that actually matters: two maps built the same way iterate the
/// same way. Within one process this holds for a random seed too, so the
/// cross-process and cross-target guarantee is the specification test above.
#[test]
fn iteration_order_is_stable() {
    let keys = [
        "angle", "speed", "node", "health", "target", "cooldown", "state", "name",
    ];

    let order = || -> Vec<&'static str> {
        let mut map = HashMap::new();
        for (i, k) in keys.iter().enumerate() {
            map.try_insert(*k, i).unwrap();
        }
        map.iter().map(|(k, _)| *k).collect()
    };

    assert_eq!(order(), order());
}
