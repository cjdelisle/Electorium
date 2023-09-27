use crate::types::Vote;
use crate::introspector::{
    Introspector,
    VoteDelegation,
    VoteDelegationRing,
    InvalidVote,
    InvalidVoteCause,
    BestRing, BestOfRing,
    PatronSelection, PatronSelectionReason,
    DeterministicTieBreaker,
    Winner,
};

fn print_ring(ring_members: &Vec<Vec<&Vote>>, delegated_votes: u64) {
    if ring_members.len() == 0 {
        println!("    No candidates found");
        return;
    }
    for (ir, r) in ring_members.iter().enumerate() {
        if ring_members.len() > 1 {
            println!("    Ring {ir}:");
        }
        for &c in r {
            println!("    {}{}",
                if ring_members.len() > 1 { "    - " } else { "- " },
                c.voter_id,
            );
        }
    }
    println!("    With {} possible delegated votes", delegated_votes);
}

pub fn new<'a>() -> Introspector<'a> {
    let mut is = Introspector::default();
    is.subscribe((), |(),e:&VoteDelegation<'a>|{
        println!("Possible delegation of {} vote(s)", e.from.number_of_votes);
        println!("    From       : {}", e.from.voter_id);
        println!("    To         : {}", e.to.voter_id);
        if e.because_of.voter_id != e.from.voter_id {
            println!("    Because {} voted for {}", e.because_of.voter_id, e.to.voter_id);
        }
    });
    is.subscribe((), |(),e:&VoteDelegationRing|{
        println!("Vote delegation encountered a ring:");
        for v in &e.chain {
            println!("    - {} -> {}", v.voter_id, v.vote_for);
        }
        println!("    Stop at: {}", e.next.voter_id);
    });
    is.subscribe((), |(),e:&InvalidVote|{
        println!("Discarding vote from {} because: {:?}", e.vote.voter_id, match e.cause {
            InvalidVoteCause::NoVote => "They didn't vote for anyone".into(),
            InvalidVoteCause::SelfVote => "They voted for themselves".into(),
            InvalidVoteCause::UnrecognizedVote =>
                format!("They voted for [{}] which is not a voter or candidate", e.vote.vote_for),
        });
    });
    is.subscribe((), |(),e:&BestRing|{
        println!("Tenative winner(s):");
        print_ring(&e.best_rings_members, e.best_total_delegated_votes);
    });
    is.subscribe((), |(),e:&BestOfRing|{
        if e.rings_member_scores.len() < 2 {
            return;
        }
        println!("Within-Ring Tie-Breaker");
        for (v, score) in &e.rings_member_scores {
            println!("    - {} votes excluding ring: {}", v.voter_id, score);
        }
        if e.winners.len() > 1 {
            println!("    Multiple ({}) tied winners, patron selection will be skipped", e.winners.len());
        }
    });
    is.subscribe((), |(),e:&PatronSelection|{
        println!("Possible patron: {} (with {} possible votes): {}",
            e.potential_patron.voter_id,
            e.potential_patron_votes,
            match &e.selection {
                PatronSelectionReason::LoopCandidate => "NO - Already eliminated by Within-Ring Tie-Breaker".into(),
                PatronSelectionReason::NotProvidingMajority(mtb) => {
                    format!("NO - Does not provide majority of votes, would need more than {mtb}")
                }
                PatronSelectionReason::NotWillingCandidate => "NO - Not a willing candidate".into(),
                PatronSelectionReason::NotBeatingSecondBest(score, cand) => {
                    format!("NO - Can't defeat 2nd best ({} with {} possible votes)", cand.voter_id, score)
                }
                PatronSelectionReason::PatronFound => "YES - Patron found".into()
            }
        );
    });
    is.subscribe((), |(),e:&DeterministicTieBreaker|{
        println!("Deterministic Tie Breaker:");
        for (v, hash) in &e.tied_candidates {
            let mut hash8 = [0_u8; 8];
            hash8.copy_from_slice(&hash[..8]);
            println!("    - Hash {:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x} for {}",
                hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7], v.voter_id);
        }
    });
    is.subscribe((), |(), e:&Option<Winner>|{
        if let Some(e) = e.as_ref() {
            println!("The winner is: {} with a total of {} delegated votes",
                e.candidate.voter_id, e.votes);
        } else {
            println!("No winner could be found");
        }
    });
    is
}