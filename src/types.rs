#[derive(Debug)]
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
impl PartialEq for Vote {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}