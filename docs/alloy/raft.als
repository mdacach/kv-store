module raft
open util/ordering[Term] as termOrd
open util/ordering[Index] as indexOrd

// Cluster members.
sig Node {
  // The node's current election term. This term might vary between nodes.
  var currentTerm: one Term,
  // Persistent voting history for each term.
  var votedFor: Term -> lone Node,
  // Persistent log entries keyed by bounded log index.
  var log: Index -> lone LogEntry
}

// Terms are finite and ordered in the model, even though Raft terms are
// conceptually unbounded integers.
sig Term {}

// Log indexes are finite and ordered in the model.
sig Index {}

// Abstract client payloads.
sig Value {}

// A log entry records the election term in which the leader created it.
sig LogEntry {
  term: one Term,
  value: one Value
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
  candidateLastLogIndex: lone Index,
  candidateLastLogTerm: lone Term
}

// A node replies to a vote request.
sig RequestVoteResponse extends Message {
  voteGranted: one Bool
}

// A leader sends AppendEntries to one follower. This message is used both for
// heartbeats (to maintain active leadership) and for log replication (to append
// new entries to a follower's log).
sig AppendEntriesRequest extends Message {
  // Index and term of the entry immediately before the entries carried by this
  // request. Followers use this pair to decide whether their log matches the
  // leader at the splice point. Both are none when the request starts at the
  // beginning of the log.
  prevLogIndex: lone Index,
  prevLogTerm: lone Term,
  // Index and entry being replicated. Both are none for an empty heartbeat.
  // This model sends at most one entry per request.
  appendEntryIndex: lone Index,
  appendEntry: lone LogEntry
}

// A follower replies to AppendEntries with whether the request matched its log
// and, on success, the highest index made known to match the leader.
sig AppendEntriesResponse extends Message {
  appendSuccess: one Bool,
  // Highest follower log index that matches the leader after processing the
  // request. For a successful request with an appended entry, this is the
  // request's appendEntryIndex. For a successful empty heartbeat, this is the
  // request's prevLogIndex. For an empty heartbeat at the beginning of the log,
  // this is none. Later response-handling transitions use it to update the
  // leader's matchIndex for the follower.
  responseMatchIndex: lone Index
}

// Simple boolean carrier for response payloads.
abstract sig Bool {}
one sig True, False extends Bool {}

var sig Follower in Node {}

var sig Candidate in Node {
  // Servers from which this candidate has received a granted vote in its
  // current term.
  var votesGranted: set Node,
  // Servers this candidate has already asked for a vote in its current term.
  // Because this model records a self-vote during timeout, the candidate is
  // also recorded here at the start of an election.
  var votesRequested: set Node
}

var sig Leader in Node {
  // The next log index to send to each peer. A missing index means the next
  // position is outside the bounded Index scope.
  var nextIndex: Node -> lone Index,
  // The highest log index each peer has acknowledged.
  var matchIndex: Node -> lone Index
}

// Messages used in a transition must be outside the network before that step.
pred fresh[m: Message] {
  m not in InFlight
}

pred send[m: Message] { InFlight' = InFlight + m }

pred discard[m: Message] { InFlight' = InFlight - m }

pred reply[request, response: Message] { InFlight' = (InFlight - request) + response }

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
  { i: Index | some logEntry[n, i] }
}

// Log entry at a specific node/index, if present.
fun logEntry[n: Node, i: Index] : lone LogEntry {
  n.log[i]
}

// The last occupied log index for a node, if its log is non-empty.
fun lastLogIndex[n: Node] : lone Index {
  { i: logIndexes[n] |
    no later: logIndexes[n] | later in i.^(indexOrd/next)
  }
}

// Entries currently present in any node log.
fun entriesInLogs : set LogEntry {
  Index.(Node.log)
}

// First unoccupied log index after a node's contiguous log prefix, if one is
// representable in the bounded Index scope.
fun firstFreeLogIndex[n: Node] : lone Index {
  { i : Index |
    i not in logIndexes[n]
    and (i = indexOrd/first or i.(indexOrd/prev) in logIndexes[n])
  }
}

// Indexes at or after a given index.
fun indexesFrom[i: Index] : set Index {
  i + i.^(indexOrd/next)
}

fun indexAfter[i: lone Index] : lone Index {
  { next: Index |
    (no i and next = indexOrd/first)
    or (some i and next = i.(indexOrd/next))
  }
}

fun previousIndexOrFirst[i: Index] : one Index {
  i.(indexOrd/prev) + (i & indexOrd/first)
}

// Raft's log freshness rule for RequestVote. A candidate is at least as
// up-to-date as the receiver when its last log term is newer, or when terms are
// equal and its last log index is at least as large.
pred logUpToDate[candidateLastIndex: lone Index, candidateLastTerm: lone Term, receiver: Node] {
  let receiverLastIndex = lastLogIndex[receiver],
      receiverLastTerm = logEntry[receiver, receiverLastIndex].term |
    no receiverLastTerm
    or (
      some candidateLastTerm
      and (
        termGt[candidateLastTerm, receiverLastTerm]
        or (
          candidateLastTerm = receiverLastTerm
          and some candidateLastIndex
          and indexGte[candidateLastIndex, receiverLastIndex]
        )
      )
    )
}

// AppendEntries can proceed only when the receiver contains the previous log
// entry named by the leader, or when the leader is appending at the first index.
pred prevLogMatches[receiver: Node, request: AppendEntriesRequest] {
  no request.prevLogIndex
  or (
    request.prevLogIndex in logIndexes[receiver]
    and request.prevLogTerm = logEntry[receiver, request.prevLogIndex].term
  )
}

pred unchangedRoles {
  Follower' = Follower
  Candidate' = Candidate
  Leader' = Leader
}

pred unchangedTerms {
  currentTerm' = currentTerm
}

pred unchangedVoting {
  votedFor' = votedFor
  votesGranted' = votesGranted
  votesRequested' = votesRequested
}

pred unchangedNetwork {
  InFlight' = InFlight
}

pred unchangedLog {
  log' = log
}

pred unchangedLeaderBookkeeping {
  nextIndex' = nextIndex
  matchIndex' = matchIndex
}

pred clearInactiveCandidateBookkeeping {
  votesGranted' = votesGranted - ((Candidate - Candidate') -> Node)
  votesRequested' = votesRequested - ((Candidate - Candidate') -> Node)
}

pred clearInactiveLeaderBookkeeping {
  nextIndex' = nextIndex - ((Leader - Leader') -> Node -> Index)
  matchIndex' = matchIndex - ((Leader - Leader') -> Node -> Index)
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
  no votesRequested

  // No messages are in flight initially.
  no InFlight

  // Logs start empty.
  no log

  // There are no leaders initially, so no leader-only replication bookkeeping.
  no nextIndex
  no matchIndex
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
  votesRequested' =
    (votesRequested - (n -> Node)) + (n -> n)

  // Unchanged state.
  Leader' = Leader
  // Timeouts do not directly change the network. Vote requests will come
  // as part of another transition.
  unchangedNetwork
  unchangedLog
  unchangedLeaderBookkeeping
}

// A candidate sends a RequestVoteRequest to one peer.
pred sendRequestVoteRequest[candidate, other: Node, request: RequestVoteRequest] {
  candidate in Candidate
  other != candidate
  other not in candidate.votesRequested
  fresh[request]

  request.source = candidate
  request.dest = other
  request.messageTerm = candidate.currentTerm
  let candidateLastIndex = lastLogIndex[candidate] {
    request.candidateLastLogIndex = candidateLastIndex
    request.candidateLastLogTerm = logEntry[candidate, candidateLastIndex].term
  }

  // Changed state.
  // The new message becomes in-flight.
  send[request]
  votesRequested' =
    (votesRequested - (candidate -> Node)) + (candidate -> (candidate.votesRequested + other))

  // Unchanged state.
  Follower' = Follower
  Candidate' = Candidate
  Leader' = Leader
  currentTerm' = currentTerm
  votedFor' = votedFor
  votesGranted' = votesGranted
  log' = log
  nextIndex' = nextIndex
  matchIndex' = matchIndex
}

// A higher-term RPC forces the receiver to step down and adopt the newer term.
pred higherTermStepDown[receiver: Node, message: Message] {
  termGt[message.messageTerm, receiver.currentTerm]

  Follower' = Follower + receiver
  Candidate' = Candidate - receiver
  Leader' = Leader - receiver
  currentTerm' =
    (currentTerm - (receiver -> Term)) + (receiver -> message.messageTerm)
}

// A vote request is granted when:
// 1. the request term matches the receiver's effective current term,
// 2. the receiver has not voted for a different node in that term, and
// 3. the candidate's log is at least as up-to-date as the receiver's log.
pred grantRequestVote[receiver: Node, request: RequestVoteRequest, response: RequestVoteResponse] {
  request.messageTerm = receiver.currentTerm'
  receiver.votedFor[request.messageTerm] in none + request.source
  logUpToDate[request.candidateLastLogIndex, request.candidateLastLogTerm, receiver]

  votedFor' = votedFor + (receiver -> request.messageTerm -> request.source)
  response.voteGranted = True
}

// All other RequestVoteRequest cases are denied:
// 1. the request term is stale or otherwise not the receiver's effective term,
// 2. the receiver already voted for a different node in that term, or
// 3. the candidate's log is stale relative to the receiver's log.
pred denyRequestVote[receiver: Node, request: RequestVoteRequest, response: RequestVoteResponse] {
  request.messageTerm != receiver.currentTerm'
  or receiver.votedFor[request.messageTerm] not in none + request.source
  or not logUpToDate[request.candidateLastLogIndex, request.candidateLastLogTerm, receiver]

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
    higherTermStepDown[receiver, request]
    and response.messageTerm = request.messageTerm
  ) or (
    // Unchanged state.
    not termGt[request.messageTerm, receiver.currentTerm]
    and unchangedRoles
    and unchangedTerms
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
  reply[request, response]

  // If the receiver stepped down from candidate state, its candidate-only
  // election bookkeeping disappears with that role.
  clearInactiveCandidateBookkeeping
  unchangedLog
  clearInactiveLeaderBookkeeping
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
  votesRequested' =
    (votesRequested - (candidate -> Node)) + (candidate -> (candidate.votesRequested + response.source))

  // Processing the response consumes it from the network.
  discard[response]

  // Unchanged state.
  unchangedRoles
  unchangedTerms
  votedFor' = votedFor
  unchangedLog
  unchangedLeaderBookkeeping
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
  unchangedTerms
  votedFor' = votedFor
  votesGranted' = votesGranted - (candidate -> Node)
  votesRequested' = votesRequested - (candidate -> Node)
  unchangedNetwork
  unchangedLog
  nextIndex' = nextIndex + (candidate -> Node -> firstFreeLogIndex[candidate])
  matchIndex' = matchIndex
}

// A leader receives a new entry and appends it to its local log.
pred clientAppend[leader: Node, entry: LogEntry] {
  leader in Leader
  let nextLogIndex = firstFreeLogIndex[leader] {
    some nextLogIndex
    // Client appends create a fresh log entry. Later replication transitions may
    // copy an existing leader entry into follower logs.
    entry not in entriesInLogs
    entry.term = leader.currentTerm

    // Changed state.
    log' = log + (leader -> nextLogIndex -> entry)

    // Unchanged state.
    unchangedRoles
    unchangedTerms
    unchangedVoting
    unchangedNetwork
    unchangedLeaderBookkeeping
  }
}

// A leader sends AppendEntries to one peer. The leader uses nextIndex[other] to
// describe the follower's expected next position:
//
// - prevLogIndex/prevLogTerm identify the entry just before that position;
// - appendEntryIndex/appendEntry carry the next leader entry at that position,
//   if the leader has one in the bounded log;
// - an empty appendEntry is a heartbeat or an out-of-bounds replication attempt.
//
// This transition only sends the request. It does not update nextIndex or
// matchIndex; those change later when the leader handles the follower's
// AppendEntriesResponse.
pred sendAppendEntriesRequest[leader, other: Node, request: AppendEntriesRequest] {
  leader in Leader
  other != leader
  some leader.nextIndex[other]
  fresh[request]

  request.source = leader
  request.dest = other
  request.messageTerm = leader.currentTerm
  request.prevLogIndex = leader.nextIndex[other].(indexOrd/prev)
  request.prevLogTerm = request.prevLogIndex.(leader.log).term
  request.appendEntryIndex = leader.nextIndex[other] & logIndexes[leader]
  request.appendEntry = request.appendEntryIndex.(leader.log)

  // Changed state.
  send[request]

  // Unchanged state.
  unchangedRoles
  unchangedTerms
  unchangedVoting
  unchangedLog
  unchangedLeaderBookkeeping
}

// A higher-term AppendEntries request has the same term effect as any other
// higher-term RPC: the receiver adopts the term and steps down before handling
// the request payload.
pred appendEntriesHigherTermRoleEffect[receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse] {
  higherTermStepDown[receiver, request]
  response.messageTerm = request.messageTerm
}

// A same-term AppendEntries from the current leader forces a candidate or old
// leader back to follower without changing the receiver's term.
pred appendEntriesSameTermStepDown[receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse] {
  request.messageTerm = receiver.currentTerm
  receiver not in Follower

  Follower' = Follower + receiver
  Candidate' = Candidate - receiver
  Leader' = Leader - receiver
  currentTerm' = currentTerm
  response.messageTerm = receiver.currentTerm
}

// AppendEntries leaves role and term state unchanged when the request is stale,
// or when the request is same-term and the receiver is already a follower.
pred appendEntriesRoleTermUnchanged[receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse] {
  // This is the remaining non-higher-term case: either the request is stale, or
  // it is same-term and the receiver is already a follower. A plain
  // `not termGte[...]` would lose the same-term follower case.
  not termGt[request.messageTerm, receiver.currentTerm]
  (request.messageTerm != receiver.currentTerm or receiver in Follower)

  unchangedRoles
  unchangedTerms
  response.messageTerm = receiver.currentTerm
}

// Applies exactly one AppendEntries role/term case before log validation.
pred appendEntriesRoleTermEffect[receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse] {
  (
    appendEntriesHigherTermRoleEffect[receiver, request, response]
    or appendEntriesSameTermStepDown[receiver, request, response]
    or appendEntriesRoleTermUnchanged[receiver, request, response]
  )
}

// Accepts an empty AppendEntries request. This is a heartbeat, or a request that
// proves only the previous-log index already matches.
pred acceptAppendEntriesHeartbeat[request: AppendEntriesRequest, response: AppendEntriesResponse] {
  no request.appendEntryIndex
  no request.appendEntry
  response.responseMatchIndex = request.prevLogIndex
  unchangedLog
}

// Accepts an AppendEntries request whose entry already exists at the receiver
// with the same term, leaving the log unchanged.
pred acceptAppendEntriesExistingEntry[receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse] {
  some request.appendEntryIndex
  some request.appendEntry
  logEntry[receiver, request.appendEntryIndex].term = request.appendEntry.term

  response.responseMatchIndex = request.appendEntryIndex
  unchangedLog
}

// Repairs a conflicting receiver log entry by deleting the conflicting suffix
// and writing the leader's entry at the requested index.
pred replaceAppendEntriesConflictingEntry[receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse] {
  some request.appendEntryIndex
  some request.appendEntry
  some logEntry[receiver, request.appendEntryIndex]
  logEntry[receiver, request.appendEntryIndex].term != request.appendEntry.term

  response.responseMatchIndex = request.appendEntryIndex
  log' =
    (log - (receiver -> indexesFrom[request.appendEntryIndex] -> LogEntry))
    + (receiver -> request.appendEntryIndex -> request.appendEntry)
}

// Appends the leader's entry when the receiver is missing that index and the
// index is exactly the next position in the receiver's contiguous log prefix.
pred appendAppendEntriesNewEntry[receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse] {
  some request.appendEntryIndex
  some request.appendEntry
  no logEntry[receiver, request.appendEntryIndex]
  request.appendEntryIndex = firstFreeLogIndex[receiver]

  response.responseMatchIndex = request.appendEntryIndex
  log' = log + (receiver -> request.appendEntryIndex -> request.appendEntry)
}

pred appendEntriesRequestGuard[receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse] {
  request in InFlight
  request.dest = receiver
  fresh[response]

  response.source = receiver
  response.dest = request.source
}

pred finishAppendEntriesRequest[request: AppendEntriesRequest, response: AppendEntriesResponse] {
  reply[request, response]

  // If the receiver stepped down from candidate or leader state, its
  // role-specific bookkeeping disappears with that role.
  votedFor' = votedFor
  clearInactiveCandidateBookkeeping
  clearInactiveLeaderBookkeeping
}

pred rejectStaleAppendEntriesRequest[receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse] {
  appendEntriesRequestGuard[receiver, request, response]

  termGt[receiver.currentTerm, request.messageTerm]
  appendEntriesRoleTermUnchanged[receiver, request, response]

  response.appendSuccess = False
  no response.responseMatchIndex
  unchangedLog
  finishAppendEntriesRequest[request, response]
}

pred rejectAppendEntriesPrevMismatch[receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse] {
  appendEntriesRequestGuard[receiver, request, response]

  appendEntriesRoleTermEffect[receiver, request, response]
  request.messageTerm = receiver.currentTerm'
  not prevLogMatches[receiver, request]

  response.appendSuccess = False
  no response.responseMatchIndex
  unchangedLog
  finishAppendEntriesRequest[request, response]
}

pred acceptAppendEntriesHeartbeatRequest[receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse] {
  appendEntriesRequestGuard[receiver, request, response]

  appendEntriesRoleTermEffect[receiver, request, response]
  request.messageTerm = receiver.currentTerm'
  prevLogMatches[receiver, request]
  response.appendSuccess = True
  acceptAppendEntriesHeartbeat[request, response]

  finishAppendEntriesRequest[request, response]
}

pred acceptAppendEntriesExistingEntryRequest[receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse] {
  appendEntriesRequestGuard[receiver, request, response]

  appendEntriesRoleTermEffect[receiver, request, response]
  request.messageTerm = receiver.currentTerm'
  prevLogMatches[receiver, request]
  response.appendSuccess = True
  acceptAppendEntriesExistingEntry[receiver, request, response]

  finishAppendEntriesRequest[request, response]
}

pred replaceAppendEntriesConflictRequest[receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse] {
  appendEntriesRequestGuard[receiver, request, response]

  appendEntriesRoleTermEffect[receiver, request, response]
  request.messageTerm = receiver.currentTerm'
  prevLogMatches[receiver, request]
  response.appendSuccess = True
  replaceAppendEntriesConflictingEntry[receiver, request, response]

  finishAppendEntriesRequest[request, response]
}

pred appendAppendEntriesNewEntryRequest[receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse] {
  appendEntriesRequestGuard[receiver, request, response]

  appendEntriesRoleTermEffect[receiver, request, response]
  request.messageTerm = receiver.currentTerm'
  prevLogMatches[receiver, request]
  response.appendSuccess = True
  appendAppendEntriesNewEntry[receiver, request, response]

  finishAppendEntriesRequest[request, response]
}

// A server handles AppendEntries by selecting one complete request-handling
// case. Each case includes the guard, role/term effect, log result, response,
// network update, and frames.
pred handleAppendEntriesRequest[receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse] {
  rejectStaleAppendEntriesRequest[receiver, request, response]
  or rejectAppendEntriesPrevMismatch[receiver, request, response]
  or acceptAppendEntriesHeartbeatRequest[receiver, request, response]
  or acceptAppendEntriesExistingEntryRequest[receiver, request, response]
  or replaceAppendEntriesConflictRequest[receiver, request, response]
  or appendAppendEntriesNewEntryRequest[receiver, request, response]
}

pred appendEntriesResponseGuard[receiver: Node, response: AppendEntriesResponse] {
  response in InFlight
  response.dest = receiver
}

pred finishAppendEntriesResponse[response: AppendEntriesResponse] {
  discard[response]
  unchangedLog
}

pred higherTermAppendEntriesResponseStepDown[receiver: Node, response: AppendEntriesResponse] {
  appendEntriesResponseGuard[receiver, response]
  higherTermStepDown[receiver, response]

  unchangedVoting
  clearInactiveLeaderBookkeeping
  finishAppendEntriesResponse[response]
}

pred dropAppendEntriesResponse[receiver: Node, response: AppendEntriesResponse] {
  appendEntriesResponseGuard[receiver, response]
  not termGt[response.messageTerm, receiver.currentTerm]
  (receiver not in Leader or response.messageTerm != receiver.currentTerm)

  unchangedRoles
  unchangedTerms
  unchangedVoting
  unchangedLeaderBookkeeping
  finishAppendEntriesResponse[response]
}

pred handleSuccessfulAppendEntriesResponse[leader: Node, response: AppendEntriesResponse] {
  appendEntriesResponseGuard[leader, response]
  leader in Leader
  response.messageTerm = leader.currentTerm
  response.appendSuccess = True
  no response.responseMatchIndex or response.responseMatchIndex in logIndexes[leader]

  unchangedRoles
  unchangedTerms
  unchangedVoting
  nextIndex' =
    (nextIndex - (leader -> response.source -> Index))
    + (leader -> response.source -> indexAfter[response.responseMatchIndex])
  (
    no response.responseMatchIndex
    and matchIndex' = matchIndex
  ) or (
    some response.responseMatchIndex
    and matchIndex' =
      (matchIndex - (leader -> response.source -> Index))
      + (leader -> response.source -> response.responseMatchIndex)
  )
  finishAppendEntriesResponse[response]
}

pred handleFailedAppendEntriesResponse[leader: Node, response: AppendEntriesResponse] {
  appendEntriesResponseGuard[leader, response]
  leader in Leader
  response.messageTerm = leader.currentTerm
  response.appendSuccess = False
  some leader.nextIndex[response.source]

  unchangedRoles
  unchangedTerms
  unchangedVoting
  nextIndex' =
    (nextIndex - (leader -> response.source -> Index))
    + (leader -> response.source -> previousIndexOrFirst[leader.nextIndex[response.source]])
  matchIndex' = matchIndex
  finishAppendEntriesResponse[response]
}

pred handleAppendEntriesResponse[receiver: Node, response: AppendEntriesResponse] {
  higherTermAppendEntriesResponseStepDown[receiver, response]
  or dropAppendEntriesResponse[receiver, response]
  or handleSuccessfulAppendEntriesResponse[receiver, response]
  or handleFailedAppendEntriesResponse[receiver, response]
}

pred receive[m: Message] {
  (some request: RequestVoteRequest, response: RequestVoteResponse |
    m = request and handleRequestVoteRequest[request.dest, request, response])
  or (some response: RequestVoteResponse |
    m = response and handleRequestVoteResponse[response.dest, response])
  or (some request: AppendEntriesRequest, response: AppendEntriesResponse |
    m = request and handleAppendEntriesRequest[request.dest, request, response])
  or (some response: AppendEntriesResponse |
    m = response and handleAppendEntriesResponse[response.dest, response])
}

// Election-related protocol actions.
pred electionActs {
  (some n: Node | timeout[n])
  or (some candidate, other: Node, request: RequestVoteRequest |
    sendRequestVoteRequest[candidate, other, request])
  or (some candidate: Node | becomeLeader[candidate])
}

// Client-facing protocol actions.
pred clientActs {
  some leader: Node, entry: LogEntry | clientAppend[leader, entry]
}

// Log-replication protocol actions.
pred replicationActs {
  some leader, other: Node, request: AppendEntriesRequest |
    sendAppendEntriesRequest[leader, other, request]
}

pred messageActs {
  some m: InFlight | receive[m]
}

pred protocolActs {
  electionActs or clientActs or replicationActs or messageActs
}

// A no-op transition to allow for lasso traces.
pred stutter {
  // No state changes.
  unchangedRoles
  unchangedTerms
  unchangedVoting
  unchangedNetwork
  unchangedLog
  unchangedLeaderBookkeeping
}

// Temporal behavior for the current scaffold.
fact traces {
  init
  always (protocolActs or stutter)
}
