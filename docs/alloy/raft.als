open util/ordering[Term] as termOrd

// Cluster members.
sig Node {
  // The node's current election term. This term might vary between nodes.
  var currentTerm: one Term,
  // Persistent voting history for each term.
  var votedFor: Term -> lone Node,
  // Servers from which this node has received a granted vote in its current term.
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
  no votesGranted

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
    // Remove `n`'s old term mapping, then add the next-term mapping.
    (currentTerm - (n -> Term)) + (n -> n.currentTerm.(termOrd/next))

  // Timing out starts a fresh election for this node, so any old response
  // bookkeeping for that node is cleared.
  votedFor' = votedFor
  votesGranted' = votesGranted - (n -> Node)
  // Timeouts do not directly change the network. Vote requests will come
  // as part of another transition.
  InFlight' = InFlight
}

// A candidate sends a RequestVoteRequest to a server that has not yet responded.
pred sendRequestVoteRequest[candidate, other: Node, request: RequestVoteRequest] {
  candidate in Candidate
  other != candidate
  fresh[request]

  request.source = candidate
  request.dest = other
  request.messageTerm = candidate.currentTerm

  // The new message becomes in-flight.
  InFlight' = InFlight + request

  Follower' = Follower
  Candidate' = Candidate
  Leader' = Leader
  currentTerm' = currentTerm
  votedFor' = votedFor
  votesGranted' = votesGranted
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
  votesGranted' = votesGranted

  // First decide what happens to the receiver's local role/term state.
  // If the request has a newer term, the receiver must step down and adopt that
  // newer term before considering the vote. Otherwise its role and term stay as-is.
  (
    // If the request term is higher, we step down from leader or candidate.
    termGt[request.messageTerm, receiver.currentTerm]
    and Follower' = Follower + receiver
    and Candidate' = Candidate - receiver
    and Leader' = Leader - receiver
    // And update our current term.
    and currentTerm' =
      (currentTerm - (receiver -> Term)) + (receiver -> request.messageTerm)
    and response.messageTerm = request.messageTerm
  ) or (
    // Otherwise, nothing changes.
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
    request.messageTerm = receiver.currentTerm'
    and receiver.votedFor[request.messageTerm] in none + request.source
    // Record the vote by adding one mapping for (receiver, term) -> candidate.
    and votedFor' = votedFor + (receiver -> request.messageTerm -> request.source)
    and response.voteGranted = True
  ) or (
    (
      request.messageTerm != receiver.currentTerm'
      or receiver.votedFor[request.messageTerm] not in none + request.source
    )
    and votedFor' = votedFor
    and response.voteGranted = False
  )

  // Handling a request consumes the request message and creates the response.
  InFlight' = (InFlight - request) + response
}

// A candidate receives a vote response for its current term.
pred handleRequestVoteResponse[candidate: Node, response: RequestVoteResponse] {
  candidate in Candidate
  response in InFlight
  response.dest = candidate
  response.messageTerm = candidate.currentTerm

  Follower' = Follower
  Candidate' = Candidate
  Leader' = Leader
  currentTerm' = currentTerm
  votedFor' = votedFor

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
}

// A no-op transition to allow for lasso traces.
pred stutter {
  Follower' = Follower
  Candidate' = Candidate
  Leader' = Leader
  currentTerm' = currentTerm
  votedFor' = votedFor
  votesGranted' = votesGranted
  InFlight' = InFlight
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
  )
}

run voteExchangeTrace {
  #Node = 3
  #Term >= 2
  eventually some RequestVoteRequest & InFlight
  eventually some RequestVoteResponse & InFlight
  eventually some votesGranted
} for 3 Node, 4 Term, 6 Message

check RolePartition for 3 Node, 4 Term, 6 Message
