
use crate::{Vote, compute_winner};

#[derive(Default)]
struct Votes {
    v: Vec<Vote>,
    next_voter_id: u32,
    test_name: String,
}
impl Votes {
    fn new(test_name: &str) -> Self {
        Self {
            test_name: test_name.into(),
            ..Default::default()
        }
    }
    fn reset(&mut self) {
        self.v.clear();
    }
    fn candidate(&mut self, name: &str, vote_for: &str) {
        self.v.push(Vote{
            voter_id: format!("{}/{}", self.test_name, name),
            vote_for: format!("{}/{}", self.test_name, vote_for),
            number_of_votes: 1,
            willing_candidate: true,
        });
    }
    fn voter(&mut self, vote_for: &str) {
        self.votes(vote_for, 1);
    }
    fn votes(&mut self, vote_for: &str, num_votes: u64) {
        self.v.push(Vote{
            voter_id: format!("voter#{}", self.next_voter_id),
            vote_for: format!("{}/{}", self.test_name, vote_for),
            number_of_votes: num_votes,
            willing_candidate: false,
        });
        self.next_voter_id += 1;
    }
    fn expect_win(&self, winner: &str) {
        println!("Computing winner for: {}", self.test_name);
        let mut is = crate::logging_introspector::new();
        let res = compute_winner(&self.v, &mut is);
        if res == "" {
            assert!(winner == "");
        } else {
            let winner = format!("{}/{}", self.test_name, winner);
            assert_eq!(winner, res);
        }
    }
}

#[test]
fn test_noone() {
    let mut v = Votes::new("test_noone");
    v.voter("non-existent-candidate");
    v.expect_win("");
}

#[test]
fn test_alice_alone() {
    let mut v = Votes::new("test_alice_alone");
    v.candidate("Alice", "");
    v.expect_win("Alice");

    v.reset();
    v.candidate("Alice", "Alice");
    v.expect_win("Alice");
}

#[test]
fn test_alice_bob_charlie() {
    // From the readme
    let mut v = Votes::new("test_alice_bob_charlie");
    v.candidate("Alice", "Bob");
    v.candidate("Bob", "Alice");
    v.candidate("Charlie", "Alice");
    v.votes("Bob", 2);
    v.votes("Charlie", 4);
    v.expect_win("Alice");
}

#[test]
fn charlie_is_patron() {
    let mut v = Votes::new("charlie_is_patron");
    v.candidate("Alice", "Bob");
    v.candidate("Bob", "Alice");
    v.candidate("Charlie", "Alice");
    v.votes("Bob", 1); //  bob only has 1 vote, now Alice only has 3 votes
    v.votes("Charlie", 4);
    v.expect_win("Charlie");
}

#[test]
fn ernist_is_patron() {
    let mut v = Votes::new("ernist_is_patron");
    v.candidate("Alice", "Bob");
    v.candidate("Bob", "Alice");
    v.candidate("Charlie", "Alice");
    v.candidate("Dave", "Charlie");
    v.candidate("Ernist", "Dave");
    v.votes("Bob", 1); //  bob only has 1 vote, now Alice only has 3 votes
    v.votes("Ernist", 4);
    v.expect_win("Ernist");
}