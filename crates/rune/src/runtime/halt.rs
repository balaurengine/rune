//! Balaur fork: the instruction pointers a debugger wants the VM to stop at.
//!
//! Upstream has no break support, so a debugger has to drive the executor one
//! instruction at a time. That pays a full re-entry into the run loop per
//! instruction, which makes a breakpointed script crawl. A set the VM checks
//! itself costs one predictable branch when absent and a bit test when set.

use crate::alloc::{self, Vec};

const BITS: usize = u64::BITS as usize;

/// A set of instruction pointers the VM halts before executing.
///
/// Stored as a bitset over instruction offsets, so the check in the run loop
/// is a shift and a mask rather than a hash lookup.
#[derive(Debug, Default)]
pub struct HaltSet {
    bits: Vec<u64>,
    len: usize,
}

impl HaltSet {
    /// Construct an empty set.
    pub const fn new() -> Self {
        Self {
            bits: Vec::new(),
            len: 0,
        }
    }

    /// Add an instruction pointer, growing the bitset to fit it.
    pub fn insert(&mut self, ip: usize) -> alloc::Result<()> {
        let word = ip / BITS;

        while self.bits.len() <= word {
            self.bits.try_push(0)?;
        }

        let mask = 1u64 << (ip % BITS);

        if self.bits[word] & mask == 0 {
            self.bits[word] |= mask;
            self.len = self.len.wrapping_add(1);
        }

        Ok(())
    }

    /// Test whether an instruction pointer is in the set.
    #[inline]
    pub fn contains(&self, ip: usize) -> bool {
        let word = ip / BITS;

        match self.bits.get(word) {
            Some(bits) => bits & (1u64 << (ip % BITS)) != 0,
            None => false,
        }
    }

    /// The number of instruction pointers in the set.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Test whether the set would never halt.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl HaltSet {
    /// Collect a set from every instruction pointer an iterator yields.
    pub fn from_ips<I>(iter: I) -> alloc::Result<Self>
    where
        I: IntoIterator<Item = usize>,
    {
        let mut set = Self::new();

        for ip in iter {
            set.insert(ip)?;
        }

        Ok(set)
    }
}

#[cfg(test)]
mod tests {
    use super::HaltSet;

    #[test]
    fn membership_and_growth() {
        let mut set = HaltSet::new();
        assert!(set.is_empty());
        assert!(!set.contains(0));

        set.insert(0).unwrap();
        set.insert(63).unwrap();
        set.insert(64).unwrap();
        set.insert(4096).unwrap();
        // A repeat does not double-count.
        set.insert(64).unwrap();

        assert_eq!(set.len(), 4);
        for ip in [0, 63, 64, 4096] {
            assert!(set.contains(ip), "{ip} should be present");
        }
        for ip in [1, 62, 65, 4095, 4097, 100_000] {
            assert!(!set.contains(ip), "{ip} should be absent");
        }
    }
}
