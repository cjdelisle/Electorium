# Fuzz cases
Each case has the following layout:

```
[voter] [votes] [vote_for]
```

* `voter` is the name of the candidate
* `votes` is the number of votes which the candidate has (from anonymous sources)
* `vote_for` is the candidate who this candidate is casting their vote for

Any line beginning with `#` is ignored (comment).

Voter names cannot have spaces in them.

Note that the `votes` number only counts votes from *anonymous* sources, i.e. non-candidate
voters. So if A votes for B but B has 3 votes, B will actually have 4 votes because of A.