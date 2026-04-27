open util/ordering[Term] as termOrd

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
  var votesGranted: set Node
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

// A set of votes is a quorum when it is a strict majority of the cluster.
pred hasMajority[votes: set Node] {
  gt[#votes, div[#Node, 2]]
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

  // No messages are in flight initially.
  no InFlight
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

  // Unchanged state.
  Leader' = Leader
  // Timeouts do not directly change the network. Vote requests will come
  // as part of another transition.
  InFlight' = InFlight
}

// A candidate sends a RequestVoteRequest to one peer.
pred sendRequestVoteRequest[candidate, other: Node, request: RequestVoteRequest] {
  candidate in Candidate
  other != candidate
  fresh[request]

  request.source = candidate
  request.dest = other
  request.messageTerm = candidate.currentTerm

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

  votedFor' = votedFor + (receiver -> request.messageTerm -> request.source)
  response.voteGranted = True
}

// All other RequestVoteRequest cases are denied.
pred denyRequestVote[receiver: Node, request: RequestVoteRequest, response: RequestVoteResponse] {
  request.messageTerm != receiver.currentTerm'
  or receiver.votedFor[request.messageTerm] not in none + request.source

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

  // Processing the response consumes it from the network.
  InFlight' = InFlight - response

  // Unchanged state.
  Follower' = Follower
  Candidate' = Candidate
  Leader' = Leader
  currentTerm' = currentTerm
  votedFor' = votedFor
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
  InFlight' = InFlight
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
  InFlight' = InFlight
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

// Safety: any newly elected leader must already have a quorum of granted votes.
assert LeadersRequireMajority {
  always all n: Node |
    (n not in Leader and after n in Leader) implies
      after hasMajority[n.votesGranted]
}

// Safety: becoming leader does not also change the node's current term.
assert LeadersKeepTheirElectionTerm {
  always all n: Node |
    (n not in Leader and n in Leader') implies n.currentTerm' = n.currentTerm
}

// Safety: a node may only remain leader while its term is unchanged.
assert LeadersStepDownBeforeTermChange {
  always all n: Node |
    (n in Leader and n.currentTerm' != n.currentTerm) implies n not in Leader'
}

// Safety: handling a higher-term vote request forces the receiver out of
// candidate/leader state and back to follower.
assert HigherTermRequestForcesStepDown {
  always all receiver: Node, request: RequestVoteRequest, response: RequestVoteResponse |
    (handleRequestVoteRequest[receiver, request, response]
      and termGt[request.messageTerm, receiver.currentTerm]) implies
        (receiver in Follower'
         and receiver not in Candidate'
         and receiver not in Leader')
}

// Safety: once a node records a vote for a term, that vote never changes.
assert OneVotePerNodePerTerm {
  always all n: Node, t: Term |
    some n.votedFor[t] implies n.votedFor'[t] = n.votedFor[t]
}

// Safety: there is never more than one leader in the same term.
assert AtMostOneLeaderPerTerm {
  always all t: Term | lone { n: Leader | n.currentTerm = t }
}

check RolePartition for 5 Node, 6 Term, 4 Message
check LeadersRequireMajority for 5 Node, 6 Term, 4 Message
check LeadersKeepTheirElectionTerm for 5 Node, 6 Term, 4 Message
check LeadersStepDownBeforeTermChange for 5 Node, 6 Term, 4 Message
check HigherTermRequestForcesStepDown for 5 Node, 6 Term, 4 Message
check OneVotePerNodePerTerm for 5 Node, 6 Term, 4 Message
check AtMostOneLeaderPerTerm for 5 Node, 6 Term, 4 Message


// Visualizer-only event tags.
// These are not part of the protocol state; they exist so the Alloy visualizer
// can show which transition fired in a given step.
enum Event {
  TimeoutEvent,
  SendRequestVoteRequestEvent,
  HandleRequestVoteRequestEvent,
  HandleRequestVoteResponseEvent,
  StutterEvent
}

// Visualization helpers.
//
// These functions are parameterless on purpose so the Alloy visualizer exposes
// them as derived relations. They do not affect solving; they only make traces
// easier to inspect.

// Direct network edges for in-flight vote requests.
fun inFlightRequestEdges : Node -> Node {
  { s, d : Node |
    some req : RequestVoteRequest & InFlight |
      req.source = s and req.dest = d
  }
}

// Direct network edges for in-flight granted vote responses.
fun inFlightGrantedResponseEdges : Node -> Node {
  { s, d : Node |
    some resp : RequestVoteResponse & InFlight |
      resp.source = s and resp.dest = d and resp.voteGranted = True
  }
}

// Direct network edges for in-flight denied vote responses.
fun inFlightDeniedResponseEdges : Node -> Node {
  { s, d : Node |
    some resp : RequestVoteResponse & InFlight |
      resp.source = s and resp.dest = d and resp.voteGranted = False
  }
}

// The current votes a candidate has accumulated.
fun grantedVoteEdges : Node -> Node {
  votesGranted
}

// Which transition fired in the current step, with its main node arguments.
fun timeout_happens : Event -> Node {
  { e : TimeoutEvent, n : Node | timeout[n] }
}

fun send_request_vote_request_happens : Event -> Node -> Node {
  { e : SendRequestVoteRequestEvent, c, o : Node |
    some req : RequestVoteRequest | sendRequestVoteRequest[c, o, req]
  }
}

fun handle_request_vote_request_happens : Event -> Node -> Node {
  { e : HandleRequestVoteRequestEvent, r, s : Node |
    some req : RequestVoteRequest, resp : RequestVoteResponse |
      req.source = s and handleRequestVoteRequest[r, req, resp]
  }
}

fun handle_request_vote_response_happens : Event -> Node -> Node {
  { e : HandleRequestVoteResponseEvent, c, s : Node |
    some resp : RequestVoteResponse |
      resp.source = s and handleRequestVoteResponse[c, resp]
  }
}

fun stutter_happens : set Event {
  { e : StutterEvent | stutter }
}

// The set of events visible in the current step.
fun events : set Event {
  timeout_happens.Node +
  send_request_vote_request_happens.Node.Node +
  handle_request_vote_request_happens.Node.Node +
  handle_request_vote_response_happens.Node.Node +
  stutter_happens
}
