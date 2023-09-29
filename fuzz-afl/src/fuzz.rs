use std::io::Read;

use afl::fuzz;
use electorium::VoteCounter;
use electorium::Vote;
use electorium::introspector::Introspector;
use electorium::logging_introspector;

mod names;

// Data Shape:
// [u8 VoterID][u8 VoteForID][u8 Votes]
const VOTE_WIDTH: usize = 3;

#[inline]
fn mk_id(input: u8, names: &[&'static str]) -> String {
    names[input as usize].to_owned()
}

#[inline]
fn parse_vote(data: &[u8], names: &[&'static str]) -> Vote {
    let id = data[0];
    let vf = data[1];
    let number_of_votes = data[2] as u64;
    Vote {
        voter_id: mk_id(id, names),
        vote_for: mk_id(vf, names),
        number_of_votes,
        willing_candidate: true,
    }
}

#[inline]
fn mk_votes(data: &[u8], names: &[&'static str]) -> Vec<Vote> {
    let ok_len = data.len() / VOTE_WIDTH * VOTE_WIDTH;
    let mut out = Vec::with_capacity(ok_len / VOTE_WIDTH);
    for i in (0..ok_len).step_by(VOTE_WIDTH) {
        out.push(parse_vote(&data[i..i+VOTE_WIDTH], names));
    }
    out
}

fn run_test(verbose: bool, data: &[u8], names: &[&'static str]) {
    let votes = mk_votes(data, names);
    let is = if verbose {
        logging_introspector::new()
    } else {
        Introspector::default()
    };
    if verbose {
        println!("Votes:");
        for v in &votes {
            println!("  - {} with {} votes --> {}", v.voter_id, v.number_of_votes, v.vote_for);
        }
    }
    let mut vc = VoteCounter::new(&votes, is);
    if verbose {
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
    if verbose {
        panic!("Panicing the process so it will not suspend");
    }
}

fn main() {
    let manual = std::env::args().any(|a|a == "--manual");
    let names = names::build();
    if manual {
        let mut stdin = std::io::stdin().lock();
        let mut v = Vec::new();
        stdin.read_to_end(&mut v).unwrap();
        run_test(true, &v, &names);
    } else {
        fuzz!(|data: &[u8]| {
            run_test(false, data, &names);
        });
    }
}
