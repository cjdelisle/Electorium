// SPDX-License-Identifier: MIT OR ISC
#![allow(non_camel_case_types)] // better_any derive needs this

use std::{collections::HashMap, marker::PhantomData};
use std::any::TypeId;

use better_any::{Tid, TidAble, TidExt};

use crate::types::Vote;

/// A marker trait for each struct that can be used as an introspector event.
pub trait Event<'a>: Tid<'a> {}

#[derive(Tid)]
pub struct VoteDelegation<'a> {
    pub from: &'a Vote,
    pub to: &'a Vote,
    pub because_of: &'a Vote,
}
impl<'a> Event<'a> for VoteDelegation<'a> {}

#[derive(Tid)]
pub struct VoteDelegationRing<'a> {
    pub chain: Vec<&'a Vote>,
    pub next: &'a Vote,
}
impl<'a> Event<'a> for VoteDelegationRing<'a> {}

pub enum InvalidVoteCause {
    NoVote,
    SelfVote,
    UnrecognizedVote,
    Duplicate,
}

#[derive(Tid)]
pub struct InvalidVote<'a> {
    pub cause: InvalidVoteCause,
    pub vote: &'a Vote,
}
impl<'a> Event<'a> for InvalidVote<'a> {}

#[derive(Tid)]
pub struct BestRing<'a> {
    pub best_total_delegated_votes: u64,
    pub best_rings_members: Vec<Vec<&'a Vote>>,
}
impl<'a> Event<'a> for BestRing<'a> {}

#[derive(Tid)]
pub struct BestOfRing<'a> {
    pub rings_member_scores: Vec<(&'a Vote, u64)>,
    pub winners: Vec<&'a Vote>,
}
impl<'a> Event<'a> for BestOfRing<'a> {}

pub enum PatronSelectionReason<'a> {
    /// The candidate is part of the best loop, they have already been elminiated by best-of-loop selection.
    LoopCandidate,

    /// The "candidate" is just a voter, not a real candidate
    NotWillingCandidate,

    /// The potential patron is not providing a majority of the votes to the candidate
    NotProvidingMajority(u64),

    /// The potential patron would not have enough votes to beat the second best ring,
    /// so since they can't beat second best, they lose and thus delegate their votes.
    /// The arguments are: number of votes in the 2nd best ring, and node in the 2nd best
    /// ring with that number of votes.
    NotBeatingSecondBest(u64, &'a Vote),

    /// The patron was selected
    PatronFound,
}

#[derive(Tid)]
pub struct PatronSelection<'a> {
    /// The potential patron whom we are considering
    pub potential_patron: &'a Vote,
    /// The total number of delegated votes of the potential patron
    pub potential_patron_votes: u64,
    /// The selection, whether the potential patron IS the patron, or if not, why not.
    pub selection: PatronSelectionReason<'a>,
}
impl<'a> Event<'a> for PatronSelection<'a> {}

#[derive(Tid)]
pub struct DeterministicTieBreaker<'a> {
    /// The number of total delegated votes which each of the winners received.
    pub votes: u64,
    /// The candidates who are tied with this number of votes, along with their hash
    /// of name + number of votes. These are ordered by the hash, so the first one is the
    /// final winner.
    pub tied_candidates: Vec<(&'a Vote, [u8;64])>,
}
impl<'a> Event<'a> for DeterministicTieBreaker<'a> {}

#[derive(Tid)]
pub struct DeterministicTieBreakerHash {
    /// The candidate's ID
    pub candidate: String,
    /// The bytes which are hashed for the candidate
    pub bytes: Vec<u8>,
    /// Total number of possible indirect votes
    pub total_indirect_votes: u64,
}
impl<'a> Event<'a> for DeterministicTieBreakerHash {}

#[derive(Tid)]
pub struct Winner<'a> {
    /// The candidate who finally won
    pub candidate: &'a Vote,
    /// The number of votes which they received
    pub votes: u64,
}
impl<'a> Event<'a> for Option<Winner<'a>> {}

trait Callable<'a> {
    fn call(&mut self, t: &dyn Event<'a>);
}
struct FnCallable<'a, C: 'static, R: Event<'a>> {
    f: fn(&mut C, &R),
    c: C,
    _a: PhantomData<&'a R>,
}
impl<'a, C: 'static, R: Event<'a>> Callable<'a> for FnCallable<'a, C, R> {
    fn call(&mut self, t: &dyn Event<'a>) {
        if let Some(t) = t.downcast_ref() {
            (self.f)(&mut self.c, t);
        } else {
            println!("Warning: Unable to downcast");
        }
    }
}
#[derive(Default)]
pub struct Introspector<'a> {
    handlers: HashMap<TypeId, Vec<Box<dyn Callable<'a> + 'a>>>,
}
impl<'a> Introspector<'a> {
    pub fn subscribe<C: 'static, R: Event<'a>>(&mut self, c: C, f: fn(c: &mut C, &R)) {
        let fnc = Box::new(FnCallable{ c, f, _a: PhantomData::default() });
        let id = R::id();
        if let Some(h) = self.handlers.get_mut(&id) {
            h.push(fnc);
        } else {
            self.handlers.insert(id, vec![ fnc ]);
        }
    }
    pub fn event<R: Event<'a>>(&mut self, f: impl Fn() -> R) {
        if let Some(handlers) = self.handlers.get_mut(&R::id()) {
            let r = f();
            for h in handlers {
                h.call(&r);
            }
        }
    }
}
