// SPDX-License-Identifier: MIT OR ISC

use std::collections::HashMap;

#[cfg(test)]
mod tests;

pub struct Vote {
    /// The unique ID of the voter/candidate
    pub voter_id: String,
    /// The unique ID of the candidate who they are voting for
    pub vote_for: String,
    /// How many votes they have - in a typical national election this would be 1
    /// In the case of stock companies, for instance, this would be number of shares.
    pub number_of_votes: u64,
    /// If this voter willing to also be a candidate for election?
    pub willing_candidate: bool,
}

struct Candidate {
    /// The index of the Candidate struct within the big Vec
    idx: usize,
    /// The index of the Candidate who they voted for, if any
    vote_for: Option<usize>,
    /// The index of another Candidate who voted for the same person, if any
    voting_for_same: Option<usize>,
    /// The number of direct votes that they control
    total_direct_votes: u64,
    /// The number of indirect votes which would be received if every candidate
    /// delegated their votes.
    total_indirect_votes: u64,
    /// The first candidate who voted for voted for this candidate.
    /// This and voting_for_same are used to create a linked list.
    voted_for_me: Option<usize>,

    is_willing_candidate: bool,
}

fn mk_candidates(votes: &[Vote]) -> Vec<Candidate> {
    let mut candidate_idx_by_name = HashMap::with_capacity(votes.len());
    let mut cands = Vec::with_capacity(votes.len());
    for v in votes.iter() {
        let cand = Candidate{
            idx: cands.len(),
            vote_for: None,
            voting_for_same: None,
            total_direct_votes: v.number_of_votes,
            total_indirect_votes: 0,
            voted_for_me: None,
            is_willing_candidate: v.willing_candidate,
        };
        candidate_idx_by_name.insert(&v.voter_id, cand.idx);
        cands.push(cand);
    }
    for (c, vote) in cands.iter_mut().zip(votes.iter()) {
        if vote.vote_for == "" {
            // They didn't vote
        } else if let Some(&idx) = candidate_idx_by_name.get(&vote.vote_for) {
            c.vote_for = Some(idx);
        }
    }
    cands
}

fn compute_delegated_votes(cand: &mut Vec<Candidate>) {
    let mut delegation_path = Vec::new();
    for node_id in 0..cand.len() {
        let (votes, mut vote_for) = {
            let c = &cand[node_id];
            (c.total_direct_votes, c.vote_for)
        };
        // Insert ourselves into the voted_for_me linked list
        if let Some(vote_for) = vote_for {
            cand[node_id].voting_for_same = cand[vote_for].voted_for_me;
            cand[vote_for].voted_for_me = Some(node_id);
        }
        delegation_path.clear();
        delegation_path.push(node_id);
        loop {
            vote_for = if let Some(vote_for) = vote_for {
                if delegation_path.contains(&vote_for) {
                    // It's a ring, we already delegated to them, abort.
                    break;
                }
                delegation_path.push(vote_for);

                let c_vf = &mut cand[vote_for];

                // Add the votes
                c_vf.total_indirect_votes += votes;

                // Next round
                c_vf.vote_for
            } else {
                // nobody left to delegate to, end of the line
                break;
            };
        }
    }
}

/// Returns the candidates with the best score, and the candidates with the 2nd best score
fn get_two_best_rings(
    cand: &Vec<Candidate>,
) -> (HashMap<usize, &Candidate>, HashMap<usize, &Candidate>) {
    #[derive(Default)]
    struct ScoreCand<'a> {
        score: u64,
        cand: HashMap<usize, &'a Candidate>,
    }
    let mut best = (ScoreCand::default(), ScoreCand::default());
    for c in cand {
        if !c.is_willing_candidate {
            continue;
        }
        if c.total_indirect_votes >= best.0.score {
            if c.total_indirect_votes >= best.1.score {
                if c.total_indirect_votes > best.1.score {
                    best.1.cand.clear();
                    best.1.score = c.total_indirect_votes;
                }
                best.1.cand.insert(c.idx, c);
            } else {
                if c.total_indirect_votes > best.0.score {
                    best.0.cand.clear();
                    best.0.score = c.total_indirect_votes;
                }
                best.0.cand.insert(c.idx, c);
            }
        }
    }
    (best.1.cand, best.0.cand)
}

/// Get the best candidate(s) out of the ring, i.e. the one(s) who would have the most
/// votes if the ring did not exist. Returns multiple in case of a tie.
fn best_of_ring<'a>(
    cand: &'a Vec<Candidate>,
    ring: &HashMap<usize, &'a Candidate>,
) -> Vec<&'a Candidate> {
    let mut winning_count = 0;
    let mut out = Vec::new();
    for (_, &c) in ring {
        let mut count = 0;
        if let Some(vfm) = c.voted_for_me {
            if !ring.contains_key(&vfm) {
                count += cand[vfm].total_indirect_votes;
            }
        }
        if count >= winning_count {
            if count > winning_count {
                out.clear();
                winning_count = count;
            }
            out.push(c);
        }
    }
    out
}

/// The "patron" is the candidate who is responsible for a majority of the
/// tenative_winner's votes, yet they are NOT part of a ring of voters who
/// all voted for eachother.
/// If the tenative_winner has a patron then this function will identify it.
///
/// It is impossible to have more than 1 patron because being a patron implies
/// supplying more than 50% of the votes to the candidate you voted for.
fn get_patron<'a>(
    cand: &'a Vec<Candidate>,
    tenative_winner: &Candidate,
    exclude_ring: &HashMap<usize, &Candidate>,
    mark_to_beat: u64,
) -> Option<&'a Candidate> {
    let mut next_vfm = tenative_winner.voted_for_me;
    let mut best_count = 0;
    let mut best = None;
    while let Some(vfm_id) = next_vfm {
        let vfm = &cand[vfm_id];
        next_vfm = vfm.voting_for_same;
        if exclude_ring.contains_key(&vfm_id) {
            continue;
        }
        if !vfm.is_willing_candidate {
            continue;
        }
        if vfm.total_indirect_votes <= mark_to_beat {
            continue;
        }
        if vfm.total_indirect_votes > best_count {
            best_count = vfm.total_indirect_votes;
            best = Some(vfm);
        }
    }
    best
}

fn solve_winner<'a>(
    cand: &'a Vec<Candidate>,
    tenative_winner: Vec<&'a Candidate>,
    best_ring: &HashMap<usize, &Candidate>,
    second_best_ring: HashMap<usize, &Candidate>,
) -> Vec<&'a Candidate> {
    // tenative_winner becomes THE winner, unless they got more than half of their
    // votes from one candidate (their "patron"), and that candidate alone has enough
    // votes to beat the second_best_ring.
    //
    // In case of a tie (multiple tenative winners), we don't consider their patrons
    // because no one of the patrons can possibly win by "revoking" their vote for
    // their tenative winner.

    let mut tenative_winner = if tenative_winner.len() != 1 {
        return tenative_winner;
    } else if let Some(&tenative_winner) = tenative_winner.get(0) {
        tenative_winner
    } else {
        unreachable!();
    };

    loop {
        let mark_to_beat = std::cmp::max(
            second_best_ring.iter().next()
                .map(|(_, &c)|c.total_indirect_votes).unwrap_or(0),
            tenative_winner.total_indirect_votes / 2,
        );
        let patron =
            get_patron(cand, tenative_winner, best_ring, mark_to_beat);
        if let Some(patron) = patron {
            tenative_winner = patron;
        } else {
            return vec![ tenative_winner ];
        }
    }
}

fn tie_breaker_hash(c: &Candidate, name: &str) -> [u8; 64] {
    use blake2::{Blake2b512, Digest};
    let mut hasher = Blake2b512::new();
    hasher.update(name.as_bytes());
    hasher.update(c.total_indirect_votes.to_le_bytes());
    hasher.finalize().into()
}

fn tie_breaker<'a>(winners: &Vec<&'a Candidate>, votes: &[Vote]) -> Option<&'a Candidate> {
    match winners.len() {
        0 => None,
        1 => Some(winners[0]),
        _ => {
            let mut wh = winners.iter()
                .map(|&w|{
                    let hash = tie_breaker_hash(w, &votes[w.idx].voter_id);
                    (hash, w)
                })
                .collect::<Vec<_>>();
            wh.sort_by_key(|(k,_)|k.clone());
            wh.iter().map(|(_,c)|*c).next()
        }
    }
}

pub fn compute_winner(votes: &[Vote]) -> String {
    // 1. Eliminate those who are not willing_candidates
    let mut cand = mk_candidates(votes);

    // 2. Compute all delegated votes
    compute_delegated_votes(&mut cand);

    // 3. Get the best and second best rings
    //    In the event that there are two disparate rings which are tied
    //    we break the tie by pretending they're all one ring and then getting
    //    the amount of votes each node in the rings would have if neither ring
    //    existed.
    let (best_ring, second_best_ring) =
        get_two_best_rings(&cand);

    // 4. Get the best candidate out of the best ring
    let tenative_winner = best_of_ring(&cand, &best_ring);
    // 5. Runoff the best candidate against his biggest voter(s)
    let winners = solve_winner(&cand, tenative_winner, &best_ring, second_best_ring);

    // 6. In case of a tie, resolve 
    let winner = tie_breaker(&winners, votes);

    // No winner = ""
    winner.map(|w|votes[w.idx].voter_id.clone()).unwrap_or_default()
}

fn main() {
    println!("Hello, world!");
}