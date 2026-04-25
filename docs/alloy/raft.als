// Initial scaffold for an Alloy model of Raft.
// We will add protocol state and transitions incrementally.

sig Node {}

sig Term {}

sig Message {}

// Each node must always be in exactly one Raft role.
var sig Follower, Candidate, Leader in Node {}

fact rolePartition {
  always {
    Node = Follower + Candidate + Leader
    disj[Follower, Candidate, Leader]
  }
}

// Placeholder initial-state predicate.
pred init {}

// Placeholder temporal model.
fact traces {
  init
}

run scaffold {
  some Node
} for 3 Node, 3 Term, 3 Message
