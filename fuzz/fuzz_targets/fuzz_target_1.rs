#![no_main]

use electorium::VoteCounter;
use electorium::Vote;
use electorium::introspector::Introspector;
use electorium::logging_introspector;

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary,Debug)]
struct VoteBin {
    voter_id: u8,
    vote_for: u8,
    votes: u16,
}

#[derive(Arbitrary,Debug)]
struct VotesBin(Vec<VoteBin>);

fuzz_target!(|data: VotesBin| {
    main_fun(&data.0[..])
});

#[inline]
fn mk_id(input: u8) -> (String, bool) {
    if input > 0x80 {
        (format!("voter/{input:02x}"), false)
    } else {
        (format!("cand/{input:02x}"), true)
    }
}

#[inline]
fn parse_vote(data: &VoteBin) -> Vote {
    let (voter_id, willing_candidate) = mk_id(data.voter_id);
    Vote {
        voter_id,
        vote_for: mk_id(data.vote_for).0,
        number_of_votes: data.votes as u64,
        willing_candidate,
    }
}

#[inline]
fn mk_votes(data: &[VoteBin]) -> Vec<Vote> {
    data.iter().map(|vb|parse_vote(vb)).collect()
}

fn main_fun(data: &[VoteBin]) {
    let verbose = std::env::args().any(|a|a == "-v");
    let votes = mk_votes(data);
    let is = if verbose {
        logging_introspector::new()
    } else {
        Introspector::default()
    };
    let mut vc = VoteCounter::new(&votes, is);
    if verbose {
        println!("Votes:");
        for v in &votes {
            println!("  - {} with {} votes --> {}", v.voter_id, v.number_of_votes, v.vote_for);
        }
        println!("Initial Scoring:");
        for (score, vote) in vc.iter() {
            println!("  - {} max possible score: {}", vote.voter_id, score);
        }
    }
    let win = match vc.find_winner() {
        None => { return; },
        Some(win) => win,
    };
    vc.revoke_vote(win);
    if verbose {
        println!("Winner identified: {}", win.voter_id);
        println!("With winner's vote revoked:");
        for (score, vote) in vc.iter() {
            println!("  - {} with votes {}", vote.voter_id, score);
        }
    }
    let mut best_score = 0;
    for (score, vote) in vc.iter() {
        if best_score == 0 {
            best_score = score;
        } else if score < best_score {
            println!("Projected winner: {} does not have the best score", win.voter_id);
            panic!("Projected winner does not have the best score");
        }
        if vote == win {
            return;
        }
    }
}
