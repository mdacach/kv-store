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

var sig Follower, Candidate, Leader in Node {}

// Safety property: every node should always be in exactly one Raft role.
assert RolePartition {
  always {
    Node = Follower + Candidate + Leader
    disj[Follower, Candidate, Leader]
  }
}

// Messages used in a transition must be outside the network before that step.
pred fresh[m: Message] {
  m not in InFlight
}

// Helper predicates for comparing finite ordered terms.
pred termGt[t1, t2: Term] {
  t1 in t2.^(termOrd/next)
}

pred termGte[t1, t2: Term] {
  t1 = t2 or termGt[t1, t2]
}

// Initial state for the leader-election model.
pred init {
  // All nodes begin as followers.
  Follower = Node
  no Candidate
  no Leader

  // All nodes start in the first term.
  currentTerm = Node -> termOrd/first

  // No node has voted yet.
  no votedFor

  // No messages are in flight initially.
  no InFlight
}

// A follower or candidate times out and starts a new election in the next term.
pred timeout[n: Node] {
  n in Follower + Candidate
  n.currentTerm != termOrd/last

  // Timing out moves the node into candidate state for the next term.
  Follower' = Follower - n
  Candidate' = Candidate + n
  Leader' = Leader

  // Only the timing-out node's term changes.
  currentTerm' =
    (currentTerm - (n -> Term)) + (n -> n.currentTerm.(termOrd/next))

  votedFor' = votedFor
  // Timeouts do not directly change the network. Vote requests will come
  // as part of another transition.
  InFlight' = InFlight
}

// A no-op transition to allow for lasso traces.
pred stutter {
  Follower' = Follower
  Candidate' = Candidate
  Leader' = Leader
  currentTerm' = currentTerm
  votedFor' = votedFor
  InFlight' = InFlight
}

// Temporal behavior for the current scaffold.
fact traces {
  init
  always (
    stutter
    or some n: Node | timeout[n]
  )
}

run timeoutTrace {
  some Node
  #Term >= 2
  eventually some Candidate
} for 3 Node, 4 Term, 6 Message

check RolePartition for 3 Node, 4 Term, 6 Message
