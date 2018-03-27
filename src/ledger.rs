//! The `ledger` crate provides the foundational data structures for Proof-of-History,
//! an ordered log of events in time.

use entry::{next_tick, Entry};
/// Each entry contains three pieces of data. The `num_hashes` field is the number
/// of hashes performed since the previous entry.  The `id` field is the result
/// of hashing `id` from the previous entry `num_hashes` times.  The `event`
/// field points to an Event that took place shortly after `id` was generated.
///
/// If you divide `num_hashes` by the amount of time it takes to generate a new hash, you
/// get a duration estimate since the last event. Since processing power increases
/// over time, one should expect the duration `num_hashes` represents to decrease proportionally.
/// Though processing power varies across nodes, the network gives priority to the
/// fastest processor. Duration should therefore be estimated by assuming that the hash
/// was generated by the fastest processor at the time the entry was recorded.
use hash::Hash;
use rayon::prelude::*;

/// Verifies the hashes and counts of a slice of events are all consistent.
pub fn verify_slice(entries: &[Entry], start_hash: &Hash) -> bool {
    let genesis = [Entry::new_tick(Default::default(), start_hash)];
    let event_pairs = genesis.par_iter().chain(entries).zip(entries);
    event_pairs.all(|(x0, x1)| x1.verify(&x0.id))
}

/// Create a vector of Ticks of length `len` from `start_hash` hash and `num_hashes`.
pub fn next_ticks(start_hash: &Hash, num_hashes: u64, len: usize) -> Vec<Entry> {
    let mut id = *start_hash;
    let mut ticks = vec![];
    for _ in 0..len {
        let entry = next_tick(&id, num_hashes);
        id = entry.id;
        ticks.push(entry);
    }
    ticks
}

#[cfg(test)]
mod tests {
    use super::*;
    use hash::hash;

    #[test]
    fn test_verify_slice() {
        let zero = Hash::default();
        let one = hash(&zero);
        assert!(verify_slice(&vec![], &zero)); // base case
        assert!(verify_slice(&vec![Entry::new_tick(0, &zero)], &zero)); // singleton case 1
        assert!(!verify_slice(&vec![Entry::new_tick(0, &zero)], &one)); // singleton case 2, bad
        assert!(verify_slice(&next_ticks(&zero, 0, 2), &zero)); // inductive step

        let mut bad_ticks = next_ticks(&zero, 0, 2);
        bad_ticks[1].id = one;
        assert!(!verify_slice(&bad_ticks, &zero)); // inductive step, bad
    }
}

#[cfg(all(feature = "unstable", test))]
mod bench {
    extern crate test;
    use self::test::Bencher;
    use ledger::*;

    #[bench]
    fn event_bench(bencher: &mut Bencher) {
        let start_hash = Default::default();
        let events = next_ticks(&start_hash, 10_000, 8);
        bencher.iter(|| {
            assert!(verify_slice(&events, &start_hash));
        });
    }
}
