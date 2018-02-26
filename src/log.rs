//! The `log` crate provides the foundational data structures for Proof-of-History,
//! an ordered log of events in time.

/// Each log entry contains three pieces of data. The 'num_hashes' field is the number
/// of hashes performed since the previous entry.  The 'end_hash' field is the result
/// of hashing 'end_hash' from the previous entry 'num_hashes' times.  The 'event'
/// field points to an Event that took place shortly after 'end_hash' was generated.
///
/// If you divide 'num_hashes' by the amount of time it takes to generate a new hash, you
/// get a duration estimate since the last event. Since processing power increases
/// over time, one should expect the duration 'num_hashes' represents to decrease proportionally.
/// Though processing power varies across nodes, the network gives priority to the
/// fastest processor. Duration should therefore be estimated by assuming that the hash
/// was generated by the fastest processor at the time the entry was logged.

use generic_array::GenericArray;
use generic_array::typenum::{U32, U64};
use ring::signature::Ed25519KeyPair;
pub type Sha256Hash = GenericArray<u8, U32>;
pub type PublicKey = GenericArray<u8, U32>;
pub type Signature = GenericArray<u8, U64>;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Entry {
    pub num_hashes: u64,
    pub end_hash: Sha256Hash,
    pub event: Event,
}

/// When 'event' is Tick, the event represents a simple clock tick, and exists for the
/// sole purpose of improving the performance of event log verification. A tick can
/// be generated in 'num_hashes' hashes and verified in 'num_hashes' hashes.  By logging
/// a hash alongside the tick, each tick and be verified in parallel using the 'end_hash'
/// of the preceding tick to seed its hashing.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum Event {
    Tick,
    Discovery {
        data: Sha256Hash,
    },
    Claim {
        key: PublicKey,
        data: Sha256Hash,
        sig: Signature,
    },
    Transaction {
        from: PublicKey,
        to: PublicKey,
        data: Sha256Hash,
        sig: Signature,
    },
}

impl Entry {
    /// Creates a Entry from the number of hashes 'num_hashes' since the previous event
    /// and that resulting 'end_hash'.
    pub fn new_tick(num_hashes: u64, end_hash: &Sha256Hash) -> Self {
        Entry {
            num_hashes,
            end_hash: *end_hash,
            event: Event::Tick,
        }
    }

    /// Verifies self.end_hash is the result of hashing a 'start_hash' 'self.num_hashes' times.
    /// If the event is not a Tick, then hash that as well.
    pub fn verify(self: &Self, start_hash: &Sha256Hash) -> bool {
        if let Event::Claim { key, data, sig } = self.event {
            if !verify_signature(&key, &data, &sig) {
                return false;
            }
        }
        if let Event::Transaction {
            from,
            to,
            data,
            sig,
        } = self.event
        {
            let mut sign_data = data.to_vec();
            sign_data.extend_from_slice(&to);
            if !verify_signature(&from, &sign_data, &sig) {
                return false;
            }
        }
        self.end_hash == next_hash(start_hash, self.num_hashes, &self.event)
    }
}

// Return a new ED25519 keypair
pub fn generate_keypair() -> Ed25519KeyPair {
    use ring::{rand, signature};
    use untrusted;
    let rng = rand::SystemRandom::new();
    let pkcs8_bytes = signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
    signature::Ed25519KeyPair::from_pkcs8(untrusted::Input::from(&pkcs8_bytes)).unwrap()
}

/// Return a Claim Event for the given hash and key-pair.
pub fn sign_hash(data: &Sha256Hash, keypair: &Ed25519KeyPair) -> Event {
    let sig = keypair.sign(data);
    let peer_public_key_bytes = keypair.public_key_bytes();
    let sig_bytes = sig.as_ref();
    Event::Claim {
        key: GenericArray::clone_from_slice(peer_public_key_bytes),
        data: GenericArray::clone_from_slice(data),
        sig: GenericArray::clone_from_slice(sig_bytes),
    }
}

/// Return a Transaction Event that indicates a transfer in ownership of the given hash.
pub fn transfer_hash(data: &Sha256Hash, keypair: &Ed25519KeyPair, to: PublicKey) -> Event {
    let from_public_key_bytes = keypair.public_key_bytes();
    let mut sign_data = data.to_vec();
    sign_data.extend_from_slice(&to);
    let sig = keypair.sign(&sign_data);
    let sig_bytes = sig.as_ref();
    Event::Transaction {
        from: GenericArray::clone_from_slice(from_public_key_bytes),
        to,
        data: GenericArray::clone_from_slice(data),
        sig: GenericArray::clone_from_slice(sig_bytes),
    }
}

/// Return a Sha256 hash for the given data.
pub fn hash(val: &[u8]) -> Sha256Hash {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::default();
    hasher.input(val);
    hasher.result()
}

/// Return the hash of the given hash extended with the given value.
pub fn extend_and_hash(end_hash: &Sha256Hash, ty: u8, val: &[u8]) -> Sha256Hash {
    let mut hash_data = end_hash.to_vec();
    hash_data.push(ty);
    hash_data.extend_from_slice(val);
    hash(&hash_data)
}

pub fn hash_event(end_hash: &Sha256Hash, event: &Event) -> Sha256Hash {
    match *event {
        Event::Tick => *end_hash,
        Event::Discovery { data } => extend_and_hash(end_hash, 1, &data),
        Event::Claim { key, data, sig } => {
            let mut event_data = data.to_vec();
            event_data.extend_from_slice(&sig);
            event_data.extend_from_slice(&key);
            extend_and_hash(end_hash, 2, &event_data)
        }
        Event::Transaction {
            from,
            to,
            data,
            sig,
        } => {
            let mut event_data = data.to_vec();
            event_data.extend_from_slice(&sig);
            event_data.extend_from_slice(&from);
            event_data.extend_from_slice(&to);
            extend_and_hash(end_hash, 2, &event_data)
        }
    }
}

pub fn next_hash(start_hash: &Sha256Hash, num_hashes: u64, event: &Event) -> Sha256Hash {
    let mut end_hash = *start_hash;
    for _ in 0..num_hashes {
        end_hash = hash(&end_hash);
    }
    hash_event(&end_hash, event)
}

/// Creates the next Tick Entry 'num_hashes' after 'start_hash'.
pub fn next_entry(start_hash: &Sha256Hash, num_hashes: u64, event: Event) -> Entry {
    Entry {
        num_hashes,
        end_hash: next_hash(start_hash, num_hashes, &event),
        event,
    }
}

pub fn next_entry_mut(start_hash: &mut Sha256Hash, num_hashes: u64, event: Event) -> Entry {
    let entry = next_entry(start_hash, num_hashes, event);
    *start_hash = entry.end_hash;
    entry
}

/// Creates the next Tick Entry 'num_hashes' after 'start_hash'.
pub fn next_tick(start_hash: &Sha256Hash, num_hashes: u64) -> Entry {
    next_entry(start_hash, num_hashes, Event::Tick)
}

/// Verifies the hashes and counts of a slice of events are all consistent.
pub fn verify_slice(events: &[Entry], start_hash: &Sha256Hash) -> bool {
    use rayon::prelude::*;
    let genesis = [Entry::new_tick(Default::default(), start_hash)];
    let event_pairs = genesis.par_iter().chain(events).zip(events);
    event_pairs.all(|(x0, x1)| x1.verify(&x0.end_hash))
}

/// Verifies the hashes and events serially. Exists only for reference.
pub fn verify_slice_seq(events: &[Entry], start_hash: &Sha256Hash) -> bool {
    let genesis = [Entry::new_tick(0, start_hash)];
    let mut event_pairs = genesis.iter().chain(events).zip(events);
    event_pairs.all(|(x0, x1)| x1.verify(&x0.end_hash))
}

/// Verify a signed message with the given public key.
pub fn verify_signature(peer_public_key_bytes: &[u8], msg_bytes: &[u8], sig_bytes: &[u8]) -> bool {
    use untrusted;
    use ring::signature;
    let peer_public_key = untrusted::Input::from(peer_public_key_bytes);
    let msg = untrusted::Input::from(msg_bytes);
    let sig = untrusted::Input::from(sig_bytes);
    signature::verify(&signature::ED25519, peer_public_key, msg, sig).is_ok()
}

pub fn create_entries(start_hash: &Sha256Hash, num_hashes: u64, events: &[Event]) -> Vec<Entry> {
    let mut end_hash = *start_hash;
    events
        .iter()
        .map(|event| next_entry_mut(&mut end_hash, num_hashes, event.clone()))
        .collect()
}

/// Create a vector of Ticks of length 'len' from 'start_hash' hash and 'num_hashes'.
pub fn create_ticks(start_hash: &Sha256Hash, num_hashes: u64, len: usize) -> Vec<Entry> {
    use std::iter;
    let mut end_hash = *start_hash;
    iter::repeat(Event::Tick)
        .take(len)
        .map(|event| next_entry_mut(&mut end_hash, num_hashes, event))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_verify() {
        let zero = Sha256Hash::default();
        let one = hash(&zero);
        assert!(Entry::new_tick(0, &zero).verify(&zero)); // base case
        assert!(!Entry::new_tick(0, &zero).verify(&one)); // base case, bad
        assert!(next_tick(&zero, 1).verify(&zero)); // inductive step
        assert!(!next_tick(&zero, 1).verify(&one)); // inductive step, bad
    }

    #[test]
    fn test_next_tick() {
        let zero = Sha256Hash::default();
        assert_eq!(next_tick(&zero, 1).num_hashes, 1)
    }

    fn verify_slice_generic(verify_slice: fn(&[Entry], &Sha256Hash) -> bool) {
        let zero = Sha256Hash::default();
        let one = hash(&zero);
        assert!(verify_slice(&vec![], &zero)); // base case
        assert!(verify_slice(&vec![Entry::new_tick(0, &zero)], &zero)); // singleton case 1
        assert!(!verify_slice(&vec![Entry::new_tick(0, &zero)], &one)); // singleton case 2, bad
        assert!(verify_slice(&create_ticks(&zero, 0, 2), &zero)); // inductive step

        let mut bad_ticks = create_ticks(&zero, 0, 2);
        bad_ticks[1].end_hash = one;
        assert!(!verify_slice(&bad_ticks, &zero)); // inductive step, bad
    }

    #[test]
    fn test_verify_slice() {
        verify_slice_generic(verify_slice);
    }

    #[test]
    fn test_verify_slice_seq() {
        verify_slice_generic(verify_slice_seq);
    }

    #[test]
    fn test_reorder_attack() {
        let zero = Sha256Hash::default();
        let one = hash(&zero);

        // First, verify Discovery events
        let events = [
            Event::Discovery { data: zero },
            Event::Discovery { data: one },
        ];
        let mut entries = create_entries(&zero, 0, &events);
        assert!(verify_slice(&entries, &zero));

        // Next, swap two Discovery events and ensure verification fails.
        let event0 = entries[0].event.clone();
        let event1 = entries[1].event.clone();
        entries[0].event = event1;
        entries[1].event = event0;
        assert!(!verify_slice(&entries, &zero));
    }

    #[test]
    fn test_claim() {
        let keypair = generate_keypair();
        let event0 = sign_hash(&hash(b"hello, world"), &keypair);
        let zero = Sha256Hash::default();
        let entries = create_entries(&zero, 0, &[event0]);
        assert!(verify_slice(&entries, &zero));
    }

    #[test]
    fn test_wrong_data_claim_attack() {
        let keypair = generate_keypair();
        let mut event0 = sign_hash(&hash(b"hello, world"), &keypair);
        if let Event::Claim { key, sig, .. } = event0 {
            let data = hash(b"goodbye cruel world");
            event0 = Event::Claim { key, data, sig };
        }
        let zero = Sha256Hash::default();
        let entries = create_entries(&zero, 0, &[event0]);
        assert!(!verify_slice(&entries, &zero));
    }

    #[test]
    fn test_transfer() {
        let keypair0 = generate_keypair();
        let keypair1 = generate_keypair();
        let pubkey1 = GenericArray::clone_from_slice(keypair1.public_key_bytes());
        let event0 = transfer_hash(&hash(b"hello, world"), &keypair0, pubkey1);
        let zero = Sha256Hash::default();
        let entries = create_entries(&zero, 0, &[event0]);
        assert!(verify_slice(&entries, &zero));
    }

    #[test]
    fn test_wrong_data_transfer_attack() {
        let keypair0 = generate_keypair();
        let keypair1 = generate_keypair();
        let pubkey1 = GenericArray::clone_from_slice(keypair1.public_key_bytes());
        let mut event0 = transfer_hash(&hash(b"hello, world"), &keypair0, pubkey1);
        if let Event::Transaction { from, to, sig, .. } = event0 {
            let data = hash(b"goodbye cruel world");
            event0 = Event::Transaction {
                from,
                to,
                data,
                sig,
            };
        }
        let zero = Sha256Hash::default();
        let entries = create_entries(&zero, 0, &[event0]);
        assert!(!verify_slice(&entries, &zero));
    }

    #[test]
    fn test_transfer_hijack_attack() {
        let keypair0 = generate_keypair();
        let keypair1 = generate_keypair();
        let pubkey1 = GenericArray::clone_from_slice(keypair1.public_key_bytes());
        let mut event0 = transfer_hash(&hash(b"hello, world"), &keypair0, pubkey1);
        if let Event::Transaction {
            from, data, sig, ..
        } = event0
        {
            let theif_keypair = generate_keypair();
            let to = GenericArray::clone_from_slice(theif_keypair.public_key_bytes());
            event0 = Event::Transaction {
                from,
                to,
                data,
                sig,
            };
        }
        let zero = Sha256Hash::default();
        let entries = create_entries(&zero, 0, &[event0]);
        assert!(!verify_slice(&entries, &zero));
    }
}

#[cfg(all(feature = "unstable", test))]
mod bench {
    extern crate test;
    use self::test::Bencher;
    use log::*;

    #[bench]
    fn event_bench(bencher: &mut Bencher) {
        let start_hash = Default::default();
        let events = create_ticks(&start_hash, 10_000, 8);
        bencher.iter(|| {
            assert!(verify_slice(&events, &start_hash));
        });
    }

    #[bench]
    fn event_bench_seq(bencher: &mut Bencher) {
        let start_hash = Default::default();
        let events = create_ticks(&start_hash, 10_000, 8);
        bencher.iter(|| {
            assert!(verify_slice_seq(&events, &start_hash));
        });
    }
}
