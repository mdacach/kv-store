open util/ordering[Term] as termOrd

// Cluster members.
sig Node {
  // The node's current election term. This term might vary between nodes.
  var currentTerm: one Term,
  // Persistent voting history for each term.
  var votedFor: Term -> lone Node
}

// Terms are finite and ordered in the model, even though Raft terms are
// conceptually unbounded integers.
sig Term {}

// Base shape for all messages.
abstract sig Message {
  source: one Node,
  dest: one Node,
  messageTerm: one Term
}

// Global network state.
//
// A message is "in flight" when it exists in the network and may be delivered
// to its destination by some future transition. When delivered, messages are
// removed from this set.
//
// When compared to per-node "inboxes", a global set of messages makes it easier
// to add message drop/duplication later, and keeps network state separate from
// node-local state.
var sig InFlight in Message {}

// A candidate asks another node for a vote.
sig RequestVoteRequest extends Message {}

// A node replies to a vote request.
sig RequestVoteResponse extends Message {
  voteGranted: one Bool
}

// Simple boolean carrier for response payloads.
abstract sig Bool {}
one sig True, False extends Bool {}

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
} for 3 Node, 4 Term, 6 Message
