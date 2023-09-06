// SPDX-License-Identifier: MIT OR ISC

use std::collections::HashMap;

mod types;
mod introspector;
mod logging_introspector;
#[cfg(test)]
mod tests;

use types::Vote;
use introspector::{
    Introspector,
    VoteDelegation,
    VoteDelegationRing,
    InvalidVote,
    InvalidVoteCause,
    BestRings, BestOfRing,
    PatronSelection, PatronSelectionReason,
    DeterministicTieBreaker,
    Winner,
};

#[derive(Debug)]
struct Candidate<'a> {
    /// A reference to the Vote object which corrisponds to this candidate
    vote: &'a Vote,
    /// The index of the Candidate who they voted for, if any
    vote_for: Option<usize>,
    /// The index of another Candidate who voted for the same person, if any
    voting_for_same: Option<usize>,
    /// The number of indirect votes which would be received if every candidate
    /// delegated their votes.
    total_indirect_votes: u64,
    /// The first candidate who voted for voted for this candidate.
    /// This and voting_for_same are used to create a linked list.
    voted_for_me: Option<usize>,
    /// True if this is someone who is willing to potentially win the election.
    is_willing_candidate: bool,
    /// Forms a linked list of candidates ordered by total indirect votes, descending
    /// Non-willing candidates are not included.
    next_by_total_indirect_votes: Option<usize>,
}
impl<'a> PartialEq for Candidate<'a> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

fn mk_candidates<'a, 'b: 'a>(
    votes: &'b[Vote],
    cands: &mut Vec<Candidate<'a>>,
    is: &mut Introspector<'b>,
) -> usize {
    let mut candidate_idx_by_name = HashMap::with_capacity(votes.len());
    let mut total_willing = 0;
    for &willing in [true,false].iter() {
        for v in votes.iter() {
            if v.willing_candidate != willing {
                // Pile up all of the willing candidates at the beginning
                // to reduce memory fragmentation.
                continue;
            }
            total_willing += if willing { 1 } else { 0 };
            let cand = Candidate{
                vote: v,
                vote_for: None,
                voting_for_same: None,
                // All willing candidates implicitly vote for themselves
                total_indirect_votes: if v.willing_candidate { v.number_of_votes } else { 0 },
                voted_for_me: None,
                is_willing_candidate: v.willing_candidate,
                next_by_total_indirect_votes: None,
            };
            candidate_idx_by_name.insert(&v.voter_id, cands.len());
            cands.push(cand);
        }
    }
    for (c, vote) in cands.iter_mut().zip(votes.iter()) {
        if vote.vote_for == "" {
            // They didn't vote
            is.event(||InvalidVote{ cause: InvalidVoteCause::NoVote, vote });
        } else if vote.vote_for == vote.voter_id {
            // Voted for themselves
            is.event(||InvalidVote{ cause: InvalidVoteCause::SelfVote, vote });
        } else if let Some(&idx) = candidate_idx_by_name.get(&vote.vote_for) {
            c.vote_for = Some(idx);
        } else {
            // Voted for someone that is unrecognized 
            is.event(||InvalidVote{ cause: InvalidVoteCause::UnrecognizedVote, vote });
        }
    }
    total_willing
}

fn compute_delegated_votes<'a>(cand: &mut Vec<Candidate<'a>>, is: &mut Introspector<'a>) {
    let mut delegation_path = Vec::new();
    for node_id in 0..cand.len() {
        let (mut vote_for, orig_vote) = {
            let c = &cand[node_id];
            (c.vote_for, c.vote)
        };
        let votes = orig_vote.number_of_votes;
        // Insert ourselves into the voted_for_me linked list
        if let Some(vote_for) = vote_for {
            cand[node_id].voting_for_same = cand[vote_for].voted_for_me;
            cand[vote_for].voted_for_me = Some(node_id);
        }
        delegation_path.clear();
        delegation_path.push(node_id);
        let mut last_vote = orig_vote;
        loop {
            vote_for = if let Some(vote_for) = vote_for {
                let c_vf = &mut cand[vote_for];
                if delegation_path.contains(&vote_for) {
                    is.event(||VoteDelegationRing {
                        chain: delegation_path.iter().map(|&id|cand[id].vote).collect(),
                        next: cand[vote_for].vote,
                    });
                    // It's a ring, we already delegated to them, abort.
                    break;
                }
                is.event(||VoteDelegation {
                    from: &orig_vote,
                    to: &c_vf.vote,
                    because_of: last_vote,
                });

                delegation_path.push(vote_for);

                // Add the votes
                c_vf.total_indirect_votes += votes;

                // Next round
                last_vote = c_vf.vote;
                c_vf.vote_for
            } else {
                // nobody left to delegate to, end of the line
                break;
            };
        }
    }
}

/// Link-list the Candidates by # of votes, return the index of the candidate w/ max votes (first)
fn order_by_total_indirect<'b,'a:'b>(
    cand: &'b mut Vec<Candidate<'a>>,
    is: &mut Introspector<'a>,
    total_willing_candidates: usize,
) -> Option<usize> {
    struct Sortable {
        idx: usize,
        score: u64,
    }
    let mut sortable = Vec::with_capacity(total_willing_candidates);
    for (idx, c) in (0..total_willing_candidates).zip(cand.iter()) {
        // they should have been put in order from before
        assert!(c.is_willing_candidate);
        sortable.push(Sortable{
            idx,
            score: c.total_indirect_votes,
        });
    }
    sortable.sort_by_key(|c|c.score);
    let mut si = sortable.iter();
    if let Some(first) = si.next() {
        let mut last = first;
        for s in si {
            cand[s.idx].next_by_total_indirect_votes = Some(last.idx);
            last = s;
        }
        return Some(last.idx);
    }
    None
}

/// A "ring" is potentially more than one ring, this breaks it down into the component rings.
fn compute_ring_members<'b, 'a: 'b>(ring: &HashMap<usize, &'b Candidate<'a>>) -> Vec<Vec<&'a Vote>> {
    let mut out: Vec<Vec<&Vote>> = Vec::new();
    'outer: for (_, &c) in ring {
        for r in &out {
            if r.contains(&c.vote) {
                continue 'outer;
            }
        }
        let mut real_ring = Vec::new();
        let mut x = c;
        loop {
            if real_ring.contains(&x.vote) {
                break;
            }
            real_ring.push(x.vote);
            if let Some(next_id) = x.vote_for {
                if let Some(next) = ring.get(&next_id) {
                    x = next;
                } else {
                    // incomplete ring
                    break;
                }
            } else {
                break;
            }
        }
        out.push(real_ring);
    }
    out
}

// Return the best ring
// Then return 

/// Returns the candidates with the best score, and the 1st candidate with the 2nd best score
fn get_tenative_winners<'b, 'a: 'b>(
    cand: &'b Vec<Candidate<'a>>,
    best: usize,
    is: &mut Introspector<'a>,
) -> (HashMap<usize, &'b Candidate<'a>>, Option<&'b Candidate<'a>>) {
    let mut best_ring = HashMap::new();
    let mut c_idx = best;
    let score = cand[c_idx].total_indirect_votes;
    let runner_up = loop {
        let c = &cand[c_idx];
        if c.total_indirect_votes < score {
            break Some(c);
        }
        best_ring.insert(c_idx, c);
        if let Some(next_idx) = c.next_by_total_indirect_votes {
            c_idx = next_idx;
        } else {
            break None;
        }
    };
    is.event(|| {
        BestRings{
            best_rings_members: compute_ring_members(&best_ring),
            best_total_delegated_votes: score,

            runner_up: runner_up.map(|ru|ru.vote),
            runner_up_score: runner_up.map(|ru|ru.total_indirect_votes).unwrap_or(0),
        }
    });
    (best_ring, runner_up)
}

/// Get the best candidate(s) out of the ring, i.e. the one(s) who would have the most
/// votes if the ring did not exist. Returns multiple in case of a tie.
fn best_of_ring<'b, 'a: 'b>(
    cand: &'b Vec<Candidate<'a>>,
    ring: &HashMap<usize, &'b Candidate<'a>>,
    is: &mut Introspector<'a>,
) -> Vec<&'b Candidate<'a>> {
    let mut scores = Vec::new();
    for (_, &c) in ring {
        let mut score = c.vote.number_of_votes;
        let mut maybe_vfm = c.voted_for_me;
        while let Some(vfm) = maybe_vfm {
            let c_vfm = &cand[vfm];
            if !ring.contains_key(&vfm) {
                score += c_vfm.total_indirect_votes;
            }
            maybe_vfm = c_vfm.voting_for_same;
        }
        scores.push((c, score));
    }
    let mut winning_count = 0;
    let mut out = Vec::new();
    for (c, score) in &scores {
        let score = *score;
        if score >= winning_count {
            if score > winning_count {
                out.clear();
                winning_count = score;
            }
            out.push(*c);
        }
    }
    is.event(||BestOfRing{
        rings_member_scores: 
            scores.iter().map(|(c, score)|(c.vote, *score)).collect(),
        winners:
            out.iter().map(|c|c.vote).collect(),
    });
    out
}

fn mk_patron_selection<'a>(
    c: &Candidate<'a>,
    p: &Candidate<'a>,
    selection: PatronSelectionReason<'a>,
) -> PatronSelection<'a> {
    PatronSelection{
        candidate: c.vote,
        candidate_votes: c.total_indirect_votes,
        potential_patron: p.vote,
        potential_patron_votes: p.total_indirect_votes,
        selection,
    }
}

/// The "patron" is the candidate who is responsible for a majority of the
/// tenative_winner's votes, yet they are NOT part of a ring of voters who
/// all voted for eachother.
/// If the tenative_winner has a patron then this function will identify it.
///
/// It is impossible to have more than 1 patron because being a patron implies
/// supplying more than 50% of the votes to the candidate you voted for.
fn get_patron<'b, 'a: 'b>(
    cand: &'b Vec<Candidate<'a>>,
    tenative_winner: &'b Candidate<'a>,
    exclude_ring: &HashMap<usize, &'b Candidate<'a>>,
    runner_up: &'b Candidate<'a>,
    is: &mut Introspector<'a>,
) -> Option<&'b Candidate<'a>> {
    let mut next_vfm = tenative_winner.voted_for_me;
    let mut best = None;

    while let Some(vfm_id) = next_vfm {
        let vfm = &cand[vfm_id];
        next_vfm = vfm.voting_for_same;
        if vfm.total_indirect_votes <= tenative_winner.total_indirect_votes / 2 {
            is.event(||mk_patron_selection(
                tenative_winner, vfm, PatronSelectionReason::NotProvidingMajority));
            continue;
        }
        if exclude_ring.contains_key(&vfm_id) {
            is.event(||mk_patron_selection(
                tenative_winner, vfm, PatronSelectionReason::LoopCandidate));
            continue;
        }
        if !vfm.is_willing_candidate {
            is.event(||mk_patron_selection(
                tenative_winner, vfm, PatronSelectionReason::NotWillingCandidate));
            continue;
        }

        if runner_up == vfm {
            // runner_up IS the potential patron
            // Anyone tied with them?
            match runner_up.next_by_total_indirect_votes {
                Some(idx) => {
                    let nru = &cand[idx];
                    if vfm.total_indirect_votes <= nru.total_indirect_votes {
                        // There's a tie for runner up, so patron is impossible
                        is.event(||mk_patron_selection(
                            tenative_winner, vfm, PatronSelectionReason::NotBeatingSecondBest(
                                nru.total_indirect_votes, nru.vote)));
                        continue;
                    }
                }
                None => {},
            };
        } else {
            // Simple case, does the patron have more votes than the runner up ?
            if vfm.total_indirect_votes <= runner_up.total_indirect_votes {
                is.event(||mk_patron_selection(
                    tenative_winner, vfm, PatronSelectionReason::NotBeatingSecondBest(
                        runner_up.total_indirect_votes, runner_up.vote)));
                continue;
            }
        }

        assert!(best.is_none());
        best = Some(vfm);
        is.event(||mk_patron_selection(tenative_winner, vfm, PatronSelectionReason::PatronFound));
    }
    best
}

fn solve_winner<'b, 'a: 'b>(
    cand: &'b Vec<Candidate<'a>>,
    tenative_winner: Vec<&'b Candidate<'a>>,
    best_ring: &HashMap<usize, &'b Candidate<'a>>,
    runner_up: Option<&'b Candidate<'a>>,
    is: &mut Introspector<'a>,
) -> Vec<&'b Candidate<'a>> {
    // If there's no runner-up then there can not possibly be a patron, because he
    // would count as a runner-up. So we return the winners.
    let mut runner_up = match runner_up {
        Some(ru) => ru,
        None => {
            return tenative_winner;
        }
    };

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
        tenative_winner = match get_patron(cand, tenative_winner, best_ring, runner_up, is) {
            Some(p) => p,
            None => {
                return vec![ tenative_winner ];
            }
        };
        if let Some(ru_idx) = tenative_winner.next_by_total_indirect_votes {
            runner_up = &cand[ru_idx];
        } else {
            // there's noone left!
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

fn tie_breaker<'b, 'a: 'b>(
    winners: &Vec<&'b Candidate<'a>>,
    is: &mut Introspector<'a>,
) -> Option<&'b Candidate<'a>> {
    match winners.len() {
        0 => None,
        1 => Some(winners[0]),
        _ => {
            let mut wh = winners.iter()
                .map(|&w|{
                    let hash = tie_breaker_hash(w, &w.vote.voter_id);
                    (hash, w)
                })
                .collect::<Vec<_>>();
            wh.sort_by_key(|(k,_)|k.clone());
            is.event(||DeterministicTieBreaker{
                votes: wh[0].1.total_indirect_votes,
                tied_candidates: wh.iter().map(|c|(c.1.vote, c.0)).collect(),
            });
            wh.iter().map(|(_,c)|*c).next()
        }
    }
}

pub fn compute_winner<'a>(votes: &'a [Vote], is: &mut Introspector<'a>) -> String {
    let mut cand: Vec<Candidate<'a>> = Vec::with_capacity(votes.len());

    // 1. Eliminate those who are not willing_candidates
    let total_willing_candidates = mk_candidates(votes, &mut cand, is);

    // 2. Compute all delegated votes
    compute_delegated_votes(&mut cand, is);

    let best = match order_by_total_indirect(&mut cand, is, total_willing_candidates) {
        Some(best) => best,
        None => {
            is.event(||None);
            return String::new();
        }
    };

    // 3. Get the best and second best rings
    //    In the event that there are two disparate rings which are tied
    //    we break the tie by pretending they're all one ring and then getting
    //    the amount of votes each node in the rings would have if neither ring
    //    existed.
    let (best_ring, runner_up) =
        get_tenative_winners(&cand, best, is);

    // 4. Get the best candidate out of the best ring
    let tenative_winner = best_of_ring(&cand, &best_ring, is);

    let tenative_winner =
        solve_winner(&cand, tenative_winner, &best_ring, runner_up, is);

    // 6. In case of a tie, resolve 
    let winner = tie_breaker(&tenative_winner, is);

    is.event(||winner.map(|w|Winner{ candidate: w.vote, votes: w.total_indirect_votes }));

    // No winner = ""
    winner.map(|w|w.vote.voter_id.clone()).unwrap_or_default()
}