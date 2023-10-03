use electorium::VoteCounter;
use electorium::Vote;
use electorium::introspector::Introspector;
use electorium::logging_introspector;

mod names;

// Data Shape:
// [ Flags ][ VoterID ][ VoteForID ][ Votes ]
const VOTE_WIDTH: usize = 4;

#[inline]
fn mk_id(input: u8, names: &[&'static str]) -> String {
    names[input as usize].to_owned()
}

#[inline]
fn parse_vote(data: &[u8], names: &[&'static str]) -> Vote {
    let willing_candidate = data[0] & 1 == 1;
    let id = data[1];
    let vf = data[2];
    let number_of_votes = data[3] as u64;
    Vote {
        voter_id: mk_id(id, names),
        vote_for: mk_id(vf, names),
        number_of_votes,
        willing_candidate,
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

pub struct Fuzz {
    verbose: bool,
    names: Vec<&'static str>,
}

impl Fuzz {
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose,
            names: names::build(),
        }
    }
    pub fn run(&self, data: &[u8]) -> i16 {
        let votes = mk_votes(data, &self.names);
        let is = if self.verbose {
            logging_introspector::new()
        } else {
            Introspector::default()
        };
        if self.verbose {
            println!("Votes:");
            for v in &votes {
                println!("  - {} with {} votes --> {}", v.voter_id, v.number_of_votes, v.vote_for);
            }
        }
        let mut vc = VoteCounter::new(&votes, is);
        if self.verbose {
            println!("Initial Scoring:");
            for (score, vote) in vc.iter() {
                println!("  - {} max possible score: {}", vote.voter_id, score);
            }
        }
        let win = match vc.find_winner() {
            None => { return -1; },
            Some(win) => win,
        };
        vc.revoke_vote(win);
        if self.verbose {
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
                for (i, n) in self.names.iter().enumerate() {
                    if *n == win.voter_id {
                        return i as i16;
                    }
                }
                panic!("Name {} is not present in the list", win.voter_id);
            }
        }
        return -1;
    }
}

#[no_mangle]
pub extern "C" fn electorium_fuzz_new(verbose: bool) -> *const Fuzz {
    Box::leak(Box::new(Fuzz::new(verbose)))
}

#[no_mangle]
pub extern "C" fn electorium_fuzz_destroy(f: *const Fuzz) {
    drop(unsafe { Box::from_raw(f as *mut Fuzz) });
}

#[no_mangle]
pub extern "C" fn electorium_fuzz_run(f: *const Fuzz, buf: *const u8, len: usize) -> i16 {
    let (f, dat) = unsafe {
        (
            Box::from_raw(f as *mut Fuzz),
            std::slice::from_raw_parts(buf, len),
        )
    };
    let out = f.run(dat);
    Box::leak(f);
    out
}