module scenarios
open raft

run voteExchangeTrace {
  #Node = 5
  #Term >= 2
  eventually some RequestVoteRequest & InFlight
  eventually some RequestVoteResponse & InFlight
  eventually some votesGranted
} for 5 Node, 6 Term, 4 Message

// With 5 nodes, a candidate already has its self-vote, so it needs 2 more
// votes to reach a majority of 3. Because message fields are immutable, each
// remote vote needs its own RequestVoteRequest atom and its own
// RequestVoteResponse atom, so 4 Message atoms are enough for this scope.
run leaderTrace {
  #Node = 5
  #Term >= 2
  eventually some Leader
} for 5 Node, 6 Term, 4 Message
