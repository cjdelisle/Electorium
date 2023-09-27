# Electorium: Delegated Voting System

## Objective
A voting system where you can vote for anyone you want, and there is no harm if they
don't win because your vote is simply delegated to whoever THEY voted for.

## Challenges
This objective poses a unique challenge because the order in which candidates are
eliminated, and their votes delegated, will impact who finally wins. Consider the
following example:

* Alice has 2 votes
* Bob has 3 votes
* Charlie has 4 votes

However, Alice's 2 votes are from Bob and Charlie.
If Alice is eliminated first then Bob gets 5 votes and wins. If Bob is eliminated
first then Alice gets 5 votes and wins. An immediate reaction might be to eliminate
Alice because she has the fewest votes, but the 4 people who voted for Charlie are
thus disenfranchised because their votes were not delegated to Alice, who would have
won.

But suppose one of Bob's 3 votes was from Alice: Now if Bob wins, Charlie's 4 voters
are not entirely disenfranchised because their vote *was* delegated to the eventual
winner via Charlie -> Alice -> Bob. But we can still consider them "more"
disenfranchised than they would have been if Alice had won.

So generally we want to maximize the number of people whose vote - by some sequence of
delegations - ends up with the eventual winner. But we also want to keep these
delegation paths as short as we can.

## Definition
The definition of a winner is the candidate for whom there is NO other candidate who
could beat them assuming neither one voted for the other, directly or indirectly.

### For example
* Charlie vs. Bob -> Bob gets Alice's votes, Charlie is not the winner
* Charlie vs. Alice -> Alice gets Bob's votes, Charlie is not the winner
* Bob vs. Alice -> Alice gets Charlie's votes, Bob is not the winner
* Alice remains

In the case that a group of candidates vote for eachother in a "ring", the
interpretation of this definition of "neither one voted for the other, directly or
indirectly" becomes a bit tricky. The way we resolve this is to consider the best
candidate in that ring to be the one who would have the most votes if the ring didn't
exist at all - i.e. if none of the participants had voted.

A critically important aspect of this definition is that one can never lose an
election they would otherwise have won, simply because they chose to vote. With the
only exception being if you vote, directly or indirectly for somebody who has already
voted for you.

## The Algorithm
1. Compute the most votes that any candidate could possibly get - that is, perform
    all possible delegations for everyone.
2. Sort candidates by number of votes that they could possibly receive.
3. Take all of the candidates who are tied for the "win", if there is only one then
    they are the clear winner, but if the winner voted for someone who in turn voted
    for them, then they will be tied because they are sharing all of their delegated
    votes with one another. We refer to this as the "best ring".
4. For each candidate in the best ring, compute how many votes they would have
    received if that ring did not exist (i.e. nobody in the ring had voted).
    This candidate is the Tenative Winner.
5. If a majority of the Tenative Winner's votes came from/through one candidate
    who voted for him (excluding anyone who was in the ring), we might call this
    candidate his "Patron". Without his Patron there is no way he could have won.
    If the Patron has, himself, more votes than the 2nd best ring, we consider the
    Patron to be the Tenative Winner. If the Patron also has a Patron then we
    recurse. Note that to be the Patron of a Patron, one must have more than 50% of
    the votes of the *original* Tenative Winner, Patron recursion does not work by simply
    having more than 50% of the votes of the Patron.
6. Once a Tenative Winner with no Patron has been identified, we call him the final
    winner.

## Ties

There are two types of ties that we can have, each is broken in a different way.

### Within-Ring Ties

It is possible that in stage 4, we identify multiple candidates who would have the
exact same number of votes. If this happens then we need not care about Patrons
because neither Patron of a tied winner can possibly win by revoking his vote,
because it would just cause the tie to be won by the other candidate.

For example:
* Alan gives 100 votes to Alice who gets a total of 110 votes
* Barry gives 90 votes to Bob who gets a total of 110 votes

Alan cannot win against Bob, and Barry cannot win against Alice, so in this case we
do not compute Patrons, we pick the winner from between Alice and Bob using a
deterministic tie-breaker.

The deterministic tie-breaker uses Blake2b-512 to hash the candidate's name/id
concatnated with the maximum delegated votes they could receive (as little endian u64)
and the lowest hash wins.

### Multi-Ring Ties
It is possible that in stage 2, we identify multiple rings which have exactly the same
number of votes. In this case, we use a different tie-breaker which is more convenient
to code. The way we break this tie is by treating everyone in all rings as though they
were part of the same ring, and computing - for each one - how many votes they received
excepting those from/through any other member of the rings.

This tie-breaker will break in favor of smaller rings where the participants each amass
more out-of-ring votes, but as a tie should exceedingly unlikely, it is doubtful that
stratigic voting over this behavior would make sense in any realistic application.

After a Multi-Ring tie is broken, it is possible to have a Within-Ring Tie as well,
even amongst members of different rings, and this is settled in the normal way.

NOTE: If we encounter a multi-ring tie, we do NOT search for Patrons because the Patron
of the candidates in one ring is not the Patron of the nodes in any other ring, so to
allow the Patron to win would cause the nodes in the other ring to erroniously lose.