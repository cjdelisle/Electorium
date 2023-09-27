use afl::fuzz;
use electorium::VoteCounter;
use electorium::Vote;
use electorium::introspector::Introspector;

// Data Shape:
// [u16 VoterID][u16 Votes][u16 VoteForID]
const VOTE_WIDTH: usize = 6;

#[inline]
fn mk_id(input: u16) -> (String, bool) {
    if input > 0x8000 {
        (format!("voter/{input:04x}"), false)
    } else {
        (format!("cand/{input:04x}"), true)
    }
}

#[inline]
fn parse_u16(data: &[u8]) -> u16 {
    ((data[1] as u16) << 8) | data[0] as u16
}

#[inline]
fn parse_vote(data: &[u8]) -> Vote {
    let id = parse_u16(&data[0..2]);
    let number_of_votes = parse_u16(&data[2..4]) as u64;
    let vf = parse_u16(&data[4..6]);
    let (voter_id, willing_candidate) = mk_id(id);
    Vote {
        voter_id,
        vote_for: mk_id(vf).0,
        number_of_votes,
        willing_candidate,
    }
}

#[inline]
fn mk_votes(data: &[u8]) -> Vec<Vote> {
    let ok_len = data.len() / VOTE_WIDTH * VOTE_WIDTH;
    let mut out = Vec::with_capacity(ok_len / VOTE_WIDTH);
    for i in (0..ok_len).step_by(VOTE_WIDTH) {
        out.push(parse_vote(&data[i..i+VOTE_WIDTH]));
    }
    out
}

fn main() {
    fuzz!(|data: &[u8]| {
        let votes = mk_votes(data);
        let is = Introspector::default();
        let mut vc = VoteCounter::new(&votes, is);
        let win = match vc.find_winner() {
            None => { return; },
            Some(win) => win,
        };
        vc.revoke_vote(win);
        let mut best_score = 0;
        for (score, vote) in vc.iter() {
            if best_score == 0 {
                best_score = score;
            } else if score < best_score {
                panic!("Projected winner: {} does not have the best score {}", win.voter_id, vote.voter_id);
            } else if vote == win {
                return;
            }
        }
    });
}
