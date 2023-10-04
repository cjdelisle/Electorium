// SPDX-License-Identifier: MIT OR ISC

use std::collections::HashMap;
use std::collections::BTreeMap;

mod types;
pub mod introspector;
pub mod logging_introspector;
#[cfg(test)]
mod tests;

pub use types::Vote;
use introspector::{
    Introspector,
    VoteDelegation,
    VoteDelegationRing,
    InvalidVote,
    InvalidVoteCause,
    BestRing, BestOfRing,
    PatronSelection, PatronSelectionReason,
    DeterministicTieBreaker,
    DeterministicTieBreakerHash,
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
    is: &mut Introspector<'a>,
) -> usize {
    let mut candidate_idx_by_name = HashMap::with_capacity(votes.len());
    let mut total_willing = 0;
    for &willing in [true,false].iter() {
        for v in votes.iter() {
            if v.willing_candidate != willing {
                // Pile up all of the willing candidates at the beginning
                // to reduce memory fragmentation, we also rely on this in
                // order_by_total_indirect
                continue;
            }
			if candidate_idx_by_name.contains_key(&v.voter_id) {
				is.event(||InvalidVote{ cause: InvalidVoteCause::Duplicate, vote: v });
				continue;
			}
            total_willing += if willing { 1 } else { 0 };
            let cand = Candidate{
                vote: v,
                vote_for: None,
                voting_for_same: None,
                // Everyone implicitly votes for themselves
                total_indirect_votes: v.number_of_votes,
                voted_for_me: None,
                is_willing_candidate: v.willing_candidate,
                next_by_total_indirect_votes: None,
            };
            candidate_idx_by_name.insert(&v.voter_id, cands.len());
            cands.push(cand);
        }
    }
    for c in cands.iter_mut() {
        let vote = c.vote;
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
fn compute_ring_members<'b, 'a: 'b>(
    cand: &'b Vec<Candidate<'a>>,
    ring: &BTreeMap<usize, &'b Candidate<'a>>,
) -> Vec<Vec<&'a Vote>> {
    let mut out: Vec<Vec<&Vote>> = Vec::new();
    let mut unorganized = BTreeMap::new();
    for (k, v) in ring {
        unorganized.insert(k, v);
    }
    loop {
        let mut ring = Vec::new();
        let (idx, _) = match unorganized.iter().next() {
            Some(x) => x,
            None => break,
        };
        let mut maybe_idx = Some(**idx);
        let mut orig_c = None;
        while let Some(idx) = maybe_idx {
            match unorganized.remove(&idx) {
                Some(c) => {
                    ring.push(c);
                    if orig_c.is_none() {
                        orig_c = Some(c);
                    }
                    maybe_idx = c.vote_for;
                }
                None => {
                    assert!(ring.contains(&&&cand[idx]) || !cand[idx].is_willing_candidate);
                    break;
                }
            }
        }
        if let Some(&c) = orig_c {
            let mut maybe_vfm = c.voted_for_me;
            while let Some(vfm) = maybe_vfm {
                if let Some(vfm_c) = unorganized.remove(&vfm) {
                    ring.push(vfm_c);
                    maybe_vfm = vfm_c.voted_for_me;
                } else {
                    maybe_vfm = cand[vfm].voting_for_same;
                }
            }
        }
        out.push(ring.iter().map(|c|c.vote).collect());
    }
    out
}

/// Returns the candidates with the best score
fn get_best_candidates<'b, 'a: 'b>(
    cand: &'b Vec<Candidate<'a>>,
    best: usize,
    is: &mut Introspector<'a>,
) -> (BTreeMap<usize, &'b Candidate<'a>>, usize) {
    let mut best_ring = BTreeMap::new();
    let mut c_idx = best;
    let score = cand[c_idx].total_indirect_votes;
    loop {
        let c = &cand[c_idx];
        if c.total_indirect_votes < score {
            break;
        }
        best_ring.insert(c_idx, c);
        if let Some(next_idx) = c.next_by_total_indirect_votes {
            c_idx = next_idx;
        } else {
            break;
        }
    };
    let best_rings_members = compute_ring_members(cand, &best_ring);
    let ring_count = best_rings_members.len();
    is.event(|| {
        BestRing{
            best_rings_members: compute_ring_members(cand, &best_ring),
            best_total_delegated_votes: score,
        }
    });
    (best_ring, ring_count)
}

/// Get the best candidate(s) out of the ring, i.e. the one(s) who would have the most
/// votes if the ring did not exist. Returns multiple in case of a tie.
fn best_of_ring<'b, 'a: 'b>(
    cand: &'b Vec<Candidate<'a>>,
    ring: &BTreeMap<usize, &'b Candidate<'a>>,
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
    p: &Candidate<'a>,
    selection: PatronSelectionReason<'a>,
) -> PatronSelection<'a> {
    PatronSelection{
        potential_patron: p.vote,
        potential_patron_votes: p.total_indirect_votes,
        selection,
    }
}

/// Get the first candidate who is not part of the 
fn get_runner_up<'b, 'a: 'b>(
    cand: &'b Vec<Candidate<'a>>,
    tenative_winner: &'b Candidate<'a>,
    exclude_ring: &BTreeMap<usize, &'b Candidate<'a>>,
) -> Option<&'b Candidate<'a>> {
    let mut ru_id = tenative_winner.next_by_total_indirect_votes;
    while let Some(id) = ru_id {
        let ru = &cand[id];
        if !exclude_ring.contains_key(&id) {
            return Some(ru);
        }
        ru_id = ru.next_by_total_indirect_votes;
    }
    None
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
    exclude_ring: &BTreeMap<usize, &'b Candidate<'a>>,
    is: &mut Introspector<'a>,
) -> Option<&'b Candidate<'a>> {

    let mut runner_up = get_runner_up(cand, tenative_winner, exclude_ring);

    // Get the potential patron of the current patron/candidate
    let get_potential_patron = |current: &'b Candidate<'a>| {
        let mut maybe_next_pp_id = current.voted_for_me;
        let mut best_score = 0;
        let mut best_cand = None;
        while let Some(next_pp_id) = maybe_next_pp_id {
            let next_pp = &cand[next_pp_id];
            maybe_next_pp_id = next_pp.voting_for_same;
            // We must exclude loop candidates early in the process
            if !exclude_ring.contains_key(&next_pp_id) && next_pp.total_indirect_votes > best_score {
                best_score = next_pp.total_indirect_votes;
                best_cand = Some(next_pp);
            }
        }
        best_cand
    };

    // Return true if the potential patron is a valid patron.
    // Does not check that they're not part of the excluded ring, but does all other checks.
    let mut is_valid_patron =
        |patron: &'b Candidate<'a>, runner_up: Option<&'b Candidate<'a>>|
    {
        let mark_to_beat = tenative_winner.total_indirect_votes / 2;
        if !patron.is_willing_candidate {
            is.event(||mk_patron_selection(
                patron, PatronSelectionReason::NotWillingCandidate));
            false
        } else if patron.total_indirect_votes <= mark_to_beat {
            is.event(||mk_patron_selection(
                patron, PatronSelectionReason::NotProvidingMajority(mark_to_beat)));
            false
        } else {
            if let Some(ru) = runner_up {
                if patron.total_indirect_votes <= ru.total_indirect_votes {
                    assert_ne!(patron, ru);
                    is.event(||mk_patron_selection(
                        patron, PatronSelectionReason::NotBeatingSecondBest(
                            ru.total_indirect_votes, ru.vote)));
                    false
                } else {
                    true
                }
            } else {
                true
            }
        }
    };

    // Get the potential patron of the best candidate of the ring
    let mut potential_patron = match get_potential_patron(tenative_winner) {
        None => return None,
        Some(pp) => pp,
    };

    // Enter the loop to search backwards for the best patron
    let mut patron = None;
    loop {
        // If the runner_up IS the potential patron, take the next runner_up,
        // otherwise we might end up comparing the patron to himself.
        if runner_up == Some(potential_patron) {
            runner_up = runner_up
                .map(|ru|ru.next_by_total_indirect_votes)
                .flatten()
                .map(|ru|&cand[ru]);
        }
        // If they're not valid, break out and keep what we've got
        if !is_valid_patron(potential_patron, runner_up) {
            break;
        }
        // The current candidate IS a patron, store them and see if a
        // node who voted for them is a patron.
        patron = Some(potential_patron);
        potential_patron = match get_potential_patron(potential_patron) {
            None => break,
            Some(pp) => pp,
        };
    }

    if let Some(p) = patron {
        is.event(||mk_patron_selection(p, PatronSelectionReason::PatronFound));
    }
    patron
}

fn solve_winner<'b, 'a: 'b>(
    cand: &'b Vec<Candidate<'a>>,
    tenative_winner: Vec<&'b Candidate<'a>>,
    best_ring: &BTreeMap<usize, &'b Candidate<'a>>,
    is: &mut Introspector<'a>,
) -> Vec<&'b Candidate<'a>> {

    // tenative_winner becomes THE winner, unless they got more than half of their
    // votes from one candidate (their "patron"), and that candidate alone has enough
    // votes to beat the runner_up.
    //
    // In case of a tie (multiple tenative winners), we don't consider their patrons
    // because no one of the patrons can possibly win by "revoking" their vote for
    // their tenative winner.

    let tenative_winner = if tenative_winner.len() != 1 {
        return tenative_winner;
    } else if let Some(&tenative_winner) = tenative_winner.get(0) {
        tenative_winner
    } else {
        unreachable!();
    };

    vec![
        get_patron(
            cand,
            tenative_winner,
            best_ring,
            is,
        ).unwrap_or(tenative_winner)
    ]
}

fn tie_breaker_hash<'a>(c: &Candidate, name: &str, is: &mut Introspector<'a>) -> [u8; 64] {
    use blake2::{Blake2b512, Digest};
    let mut hasher = Blake2b512::new();
    hasher.update(name.as_bytes());
    hasher.update(c.total_indirect_votes.to_le_bytes());
    let hash = hasher.finalize().into();
    is.event(||{
        let nab = name.as_bytes();
        let mut buf = vec![0_u8; nab.len() + 8];
        buf[0..nab.len()].copy_from_slice(nab);
        buf[nab.len()..].copy_from_slice(&c.total_indirect_votes.to_le_bytes()[..]);
        DeterministicTieBreakerHash{
            candidate: name.to_string(),
            total_indirect_votes: c.total_indirect_votes,
            bytes: buf,
        }
    });
    hash
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
                    let hash = tie_breaker_hash(w, &w.vote.voter_id, is);
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

pub struct VoteCounter<'a> {
    cand: Vec<Candidate<'a>>,
    is: Introspector<'a>,
    total_willing_candidates: usize,
    best: Option<usize>
}
impl<'a> VoteCounter<'a> {
    /// Create a new VoteCounter and compute the delegated votes.
    /// After this has been called, you may call iter() to
    /// walk the ranking of the candidates, or you may call find_winner to attempt to
    /// compute a winning candidate.
    pub fn new(votes: &'a [Vote], is: Introspector<'a>) -> Self {
        let mut out = VoteCounter{
            cand: Vec::with_capacity(votes.len()),
            is,
            total_willing_candidates: 0,
            best: None,
        };
        out.total_willing_candidates = mk_candidates(votes, &mut out.cand, &mut out.is);
        out.compute_delegated_votes();
        out
    }

    fn compute_delegated_votes(&mut self) {
        compute_delegated_votes(&mut self.cand, &mut self.is);
        self.best = order_by_total_indirect(&mut self.cand, self.total_willing_candidates);
    }
 
    /// Attempt to find a winning candidate using the search algorithm
    pub fn find_winner(&mut self) -> Option<&'a Vote> {
        let best = match self.best {
            Some(best) => best,
            None => {
                self.is.event(||None);
                return None
            }
        };
        let (best_ring, ring_count) = get_best_candidates(&self.cand, best, &mut self.is);

        // 4. Get the best candidate out of the best ring
        let mut tenative_winner = best_of_ring(&self.cand, &best_ring, &mut self.is);

        if ring_count < 2 {
            tenative_winner = solve_winner(&self.cand, tenative_winner, &best_ring, &mut self.is);
        }
    
        // 6. In case of a tie, resolve 
        let winner = tie_breaker(&tenative_winner, &mut self.is);
    
        self.is.event(||winner.map(|w|Winner{ candidate: w.vote, votes: w.total_indirect_votes }));
    
        winner.map(|w|w.vote)
    }

    /// Revoke a vote and re-compute, this can be used when a winning candidate has been
    /// identified to demonstrate conclusively that they are the winner - if they do not
    /// delegate their vote.
    pub fn revoke_vote(&mut self, projected_winner: &Vote) {
        for c in &mut self.cand {
            c.next_by_total_indirect_votes = None;
            c.total_indirect_votes = c.vote.number_of_votes;
            c.voted_for_me = None;
            c.voting_for_same = None;
            if c.vote == projected_winner {
                c.vote_for = None;
            }
        }
        self.best = None;
        self.compute_delegated_votes();
    }

    /// Get an iterator which yields the candidates in order by number of votes they would
    /// receive with all possible delegations.
    pub fn iter<'b>(&'b self) -> impl Iterator<Item = (u64, &'a Vote)> + 'b {
        WinnersIter{ vc: self, next: self.best }
    }
}

struct WinnersIter<'a, 'b> {
    vc: &'b VoteCounter<'a>,
    next: Option<usize>,
}
impl<'a, 'b> Iterator for WinnersIter<'a, 'b> {
    type Item = (u64, &'a Vote);
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.next {
            let cand = &self.vc.cand[next];
            self.next = cand.next_by_total_indirect_votes;
            Some((cand.total_indirect_votes, cand.vote))
        } else {
            None
        }
    }
}