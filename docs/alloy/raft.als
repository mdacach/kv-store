module raft
open util/ordering[Term] as termOrd
open util/ordering[Index] as indexOrd

// Cluster members.
sig Node {
  // The node's current election term. This term might vary between nodes.
  var currentTerm: one Term,
  // Persistent voting history for each term.
  var votedFor: Term -> lone Node,
  // Servers from which this node has received a granted vote in its current term.
  //
  // In the current model this set is only meaningful while the node is a
  // candidate. Outside candidate state it may contain stale bookkeeping left
  // over from an earlier election, but no transition consults it there.
  var votesGranted: set Node,
  // Servers from which this node has recorded a vote response in its current
  // term. Because this model records a self-vote during timeout, the candidate
  // is also recorded here at the start of an election.
  var votesResponded: set Node,
  // Persistent log entries keyed by bounded log index.
  var log: Index -> lone Entry
}

// Terms are finite and ordered in the model, even though Raft terms are
// conceptually unbounded integers.
sig Term {}

// Log indexes are finite and ordered in the model. They represent the bounded
// prefix of possible Raft log positions explored in a given Alloy scope.
sig Index {}

// Abstract client payloads. Their identity is enough for safety properties.
sig Value {}

// A log entry records the election term in which the leader created it.
sig Entry {
  entryTerm: one Term,
  entryValue: one Value
}

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
sig RequestVoteRequest extends Message {
  requestLastLogIndex: lone Index,
  requestLastLogTerm: lone Term
}

// A node replies to a vote request.
sig RequestVoteResponse extends Message {
  voteGranted: one Bool
}

// Simple boolean carrier for response payloads.
abstract sig Bool {}
one sig True, False extends Bool {}

var sig Follower, Candidate, Leader in Node {}

// Messages used in a transition must be outside the network before that step.
pred fresh[m: Message] {
  m not in InFlight
}

// Helper predicates for comparing finite ordered terms.
pred termGt[t1, t2: Term] {
  t1 in t2.^(termOrd/next)
}

pred indexGte[i1, i2: Index] {
  i1 = i2 or i1 in i2.^(indexOrd/next)
}

// A set of votes is a quorum when it is a strict majority of the cluster.
pred hasMajority[votes: set Node] {
  gt[#votes, div[#Node, 2]]
}

// Indexes currently occupied in a node's log.
fun logIndexes[n: Node] : set Index {
  n.log.Entry
}

// Entry at a specific node/index, if present.
fun logEntry[n: Node, i: Index] : lone Entry {
  i.(n.log)
}

// The last occupied log index for a node, if its log is non-empty.
fun lastLogIndex[n: Node] : lone Index {
  { i : logIndexes[n] | no i.(indexOrd/next) & logIndexes[n] }
}

// The term of the last log entry for a node, if its log is non-empty.
fun lastLogTerm[n: Node] : lone Term {
  lastLogIndex[n].(n.log).entryTerm
}

// Raft's log freshness rule for RequestVote. A candidate is at least as
// up-to-date as the receiver when its last log term is newer, or when terms are
// equal and its last log index is at least as large.
pred logUpToDate[candidateLastIndex: lone Index, candidateLastTerm: lone Term, receiver: Node] {
  no lastLogTerm[receiver]
  or (
    some candidateLastTerm
    and (
      termGt[candidateLastTerm, lastLogTerm[receiver]]
      or (
        candidateLastTerm = lastLogTerm[receiver]
        and some candidateLastIndex
        and indexGte[candidateLastIndex, lastLogIndex[receiver]]
      )
    )
  )
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
  no votesGranted
  no votesResponded

  // No messages are in flight initially.
  no InFlight

  // Logs start empty.
  no log
}

// A follower or candidate times out and starts a new election in the next term.
pred timeout[n: Node] {
  n in Follower + Candidate
  n.currentTerm != termOrd/last

  // Changed state.
  // Timing out moves the node into candidate state for the next term.
  Follower' = Follower - n
  Candidate' = Candidate + n

  // Only the timing-out node's term changes.
  currentTerm' =
    // Remove `n`'s old term mapping, then add the next-term mapping.
    (currentTerm - (n -> Term)) + (n -> n.currentTerm.(termOrd/next))

  // Timing out starts a fresh election for this node, so any old response
  // bookkeeping for that node is reset, and the node immediately records its
  // own vote for the new term.
  votedFor' =
    votedFor + (n -> n.currentTerm.(termOrd/next) -> n)
  votesGranted' =
    (votesGranted - (n -> Node)) + (n -> n)
  votesResponded' =
    (votesResponded - (n -> Node)) + (n -> n)

  // Unchanged state.
  Leader' = Leader
  // Timeouts do not directly change the network. Vote requests will come
  // as part of another transition.
  InFlight' = InFlight
  log' = log
}

// A candidate sends a RequestVoteRequest to one peer.
pred sendRequestVoteRequest[candidate, other: Node, request: RequestVoteRequest] {
  candidate in Candidate
  other != candidate
  other not in candidate.votesResponded
  fresh[request]

  request.source = candidate
  request.dest = other
  request.messageTerm = candidate.currentTerm
  request.requestLastLogIndex = lastLogIndex[candidate]
  request.requestLastLogTerm = lastLogTerm[candidate]

  // Changed state.
  // The new message becomes in-flight.
  InFlight' = InFlight + request

  // Unchanged state.
  Follower' = Follower
  Candidate' = Candidate
  Leader' = Leader
  currentTerm' = currentTerm
  votedFor' = votedFor
  votesGranted' = votesGranted
  votesResponded' = votesResponded
  log' = log
}

// A higher-term RequestVoteRequest forces the receiver to step down and adopt
// the newer term before the vote is evaluated.
pred higherTermRequestStepDown[receiver: Node, request: RequestVoteRequest, response: RequestVoteResponse] {
  termGt[request.messageTerm, receiver.currentTerm]

  Follower' = Follower + receiver
  Candidate' = Candidate - receiver
  Leader' = Leader - receiver
  currentTerm' =
    (currentTerm - (receiver -> Term)) + (receiver -> request.messageTerm)
  response.messageTerm = request.messageTerm
}

// A vote request is granted when it matches the receiver's effective current
// term and the receiver has not voted for a different node in that term.
pred grantRequestVote[receiver: Node, request: RequestVoteRequest, response: RequestVoteResponse] {
  request.messageTerm = receiver.currentTerm'
  receiver.votedFor[request.messageTerm] in none + request.source
  logUpToDate[request.requestLastLogIndex, request.requestLastLogTerm, receiver]

  votedFor' = votedFor + (receiver -> request.messageTerm -> request.source)
  response.voteGranted = True
}

// All other RequestVoteRequest cases are denied.
pred denyRequestVote[receiver: Node, request: RequestVoteRequest, response: RequestVoteResponse] {
  request.messageTerm != receiver.currentTerm'
  or receiver.votedFor[request.messageTerm] not in none + request.source
  or not logUpToDate[request.requestLastLogIndex, request.requestLastLogTerm, receiver]

  votedFor' = votedFor
  response.voteGranted = False
}

// A server handles a RequestVoteRequest.
// If the request carries a newer term, the receiver first updates its term and
// steps down to follower before deciding whether to grant the vote.
pred handleRequestVoteRequest[receiver: Node, request: RequestVoteRequest, response: RequestVoteResponse] {
  request in InFlight
  request.dest = receiver
  fresh[response]

  response.source = receiver
  response.dest = request.source

  // First decide what happens to the receiver's local role/term state.
  // If the request has a newer term, the receiver must step down and adopt that
  // newer term before considering the vote. Otherwise its role and term stay as-is.
  (
    // Changed state.
    higherTermRequestStepDown[receiver, request, response]
  ) or (
    // Unchanged state.
    not termGt[request.messageTerm, receiver.currentTerm]
    and Follower' = Follower
    and Candidate' = Candidate
    and Leader' = Leader
    and currentTerm' = currentTerm
    and response.messageTerm = receiver.currentTerm
  )

  // Then decide whether the receiver grants the vote.
  // Granting is allowed only when:
  // 1. the request term matches the receiver's effective current term, and
  // 2. the receiver has either not voted in that term or already voted for
  //    this same candidate.
  (
    // Changed state.
    grantRequestVote[receiver, request, response]
  ) or (
    // Unchanged state.
    denyRequestVote[receiver, request, response]
  )

  // Changed state.
  // Handling a request consumes the request message and creates the response.
  InFlight' = (InFlight - request) + response

  // Unchanged state.
  votesGranted' = votesGranted
  votesResponded' = votesResponded
  log' = log
}

// A candidate receives a vote response for its current term.
pred handleRequestVoteResponse[candidate: Node, response: RequestVoteResponse] {
  candidate in Candidate
  response in InFlight
  response.dest = candidate
  response.messageTerm = candidate.currentTerm

  // Changed state.
  // A granted response adds the responder to the candidate's granted-vote set.
  // The relational update removes the candidate's old vote set and replaces it
  // with the old set plus this responder.
  (
    response.voteGranted = True
    and votesGranted' =
      (votesGranted - (candidate -> Node)) + (candidate -> (candidate.votesGranted + response.source))
  ) or (
    response.voteGranted = False
    and votesGranted' = votesGranted
  )
  votesResponded' =
    (votesResponded - (candidate -> Node)) + (candidate -> (candidate.votesResponded + response.source))

  // Processing the response consumes it from the network.
  InFlight' = InFlight - response

  // Unchanged state.
  Follower' = Follower
  Candidate' = Candidate
  Leader' = Leader
  currentTerm' = currentTerm
  votedFor' = votedFor
  log' = log
}

// A candidate with a quorum of granted votes becomes leader.
pred becomeLeader[candidate: Node] {
  candidate in Candidate
  hasMajority[candidate.votesGranted]

  // Changed state.
  Candidate' = Candidate - candidate
  Leader' = Leader + candidate

  // Unchanged state.
  Follower' = Follower
  currentTerm' = currentTerm
  votedFor' = votedFor
  votesGranted' = votesGranted
  votesResponded' = votesResponded
  InFlight' = InFlight
  log' = log
}

// A no-op transition to allow for lasso traces.
pred stutter {
  // No state changes.
  Follower' = Follower
  Candidate' = Candidate
  Leader' = Leader
  currentTerm' = currentTerm
  votedFor' = votedFor
  votesGranted' = votesGranted
  votesResponded' = votesResponded
  InFlight' = InFlight
  log' = log
}

// Temporal behavior for the current scaffold.
fact traces {
  init
  always (
    stutter
    or some n: Node | timeout[n]
    or some candidate, other: Node, request: RequestVoteRequest |
      sendRequestVoteRequest[candidate, other, request]
    or some receiver: Node, request: RequestVoteRequest, response: RequestVoteResponse |
      handleRequestVoteRequest[receiver, request, response]
    or some candidate: Node, response: RequestVoteResponse |
      handleRequestVoteResponse[candidate, response]
    or some candidate: Node | becomeLeader[candidate]
  )
}


