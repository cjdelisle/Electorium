// SPDX-License-Identifier: MIT OR ISC
use crate::{Vote, VoteCounter};

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
        self.check_winner(Some(winner), true);
    }
    fn check_winner(&self, winner: Option<&str>, verbose: bool) {
        let is = if verbose {
            println!("Computing winner for: {}", self.test_name);
            crate::logging_introspector::new()
        } else {
            crate::Introspector::default()
        };
        let mut vc = VoteCounter::new(&self.v, is);
        if verbose {
            println!("Most possible votes per candidate:");
            for (votes, v) in vc.iter() {
                println!("  - {} possible votes to {}", votes, v.voter_id);
            }
        }
        let win = vc.find_winner();
        if let Some(win) = win {
            if verbose {
                println!("Projected winner is: {}", win.voter_id);
            }
            vc.revoke_vote(win);
            if verbose {
                println!("Total delegated votes with {}'s delegation removed:", win.voter_id);
                for (votes, v) in vc.iter() {
                    println!("  - {} possible votes to {}", votes, v.voter_id);
                }
            }
            let mut top = 0;
            for (votes, v) in vc.iter() {
                let t = if top == 0 {
                    top = votes;
                    votes
                } else {
                    top
                };
                assert_eq!(t, votes);
                if v == win {
                    break;
                }
            }
            if let Some(winner) = winner {
                let winner = format!("{}/{}", self.test_name, winner);
                assert_eq!(winner, win.voter_id);
            }
        } else if let Some(winner) = winner {
            assert!(winner == "");
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
    v.votes("Bob", 3);
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
    v.votes("Bob", 1); //  bob only has 1 vote, now Alice only has 2 votes
    v.votes("Ernist", 5);
    v.expect_win("Ernist");
}

#[test]
fn tennassee_capital_election() {
    let mut v = Votes::new("tennassee_capital_election");
    v.candidate("Memphis", "Nashville");
    v.candidate("Nashville", "Chattanooga");
    v.candidate("Knoxville", "Chattanooga");
    v.candidate("Chattanooga", "Knoxville");

    v.votes("Memphis", 42_000);
    v.votes("Nashville", 26_000);
    v.votes("Knoxville", 15_000);
    v.votes("Chattanooga", 17_000);

    v.expect_win("Nashville");
}