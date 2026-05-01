module scenarios
open visualization

run voteExchangeTrace {
  #Node = 5
  #Term >= 2
  eventually some RequestVoteRequest & InFlight
  eventually some RequestVoteResponse & InFlight
  eventually some votesGranted
} for 5 Node, 6 Term, 4 Message, 4 Index, 4 LogEntry, 2 Value

// With 5 nodes, a candidate already has its self-vote, so it needs 2 more
// votes to reach a majority of 3. Because message fields are immutable, each
// remote vote needs its own RequestVoteRequest atom and its own
// RequestVoteResponse atom, so 4 Message atoms are enough for this scope.
run leaderTrace {
  #Node = 5
  #Term >= 2
  eventually some Leader
} for 5 Node, 6 Term, 4 Message, 4 Index, 4 LogEntry, 2 Value

run leaderAppendTrace {
  #Node = 3
  #Term >= 2
  eventually some Leader
  eventually some n: Node | some logIndexes[n]
} for 3 Node, 4 Term, 2 Message, 3 Index, 3 LogEntry, 2 Value

run appendEntriesSendTrace {
  #Node = 3
  #Term >= 2
  eventually some AppendEntriesRequest & InFlight
} for 3 Node, 4 Term, 3 Message, 3 Index, 3 LogEntry, 2 Value

run appendEntriesReplicationTrace {
  #Node = 3
  #Term >= 2
  eventually some disj leader, follower: Node, i: Index |
    leader in Leader
    and some logEntry[leader, i]
    and logEntry[leader, i] = logEntry[follower, i]
    and leader.matchIndex[follower] = i
} for 3 Node, 4 Term, 4 Message, 3 Index, 3 LogEntry, 2 Value

run appendEntriesRequestSuccessTrace {
  #Node = 3
  #Term >= 2
  eventually some receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse |
    handleAppendEntriesRequest[receiver, request, response]
    and response.appendSuccess = True
    and some response.responseMatchIndex
} for 3 Node, 4 Term, 4 Message, 3 Index, 3 LogEntry, 2 Value

run appendEntriesStaleRejectTrace {
  #Node = 3
  #Term >= 3
  eventually some receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse |
    rejectStaleAppendEntriesRequest[receiver, request, response]
} for 3 Node, 5 Term, 5 Message, 3 Index, 3 LogEntry, 2 Value

run appendEntriesPrevMismatchRejectTrace {
  #Node = 3
  #Term >= 3
  eventually some receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse |
    rejectAppendEntriesPrevMismatch[receiver, request, response]
} for 3 Node, 5 Term, 6 Message, 3 Index, 4 LogEntry, 2 Value

run appendEntriesConflictRepairTrace {
  #Node = 3
  #Term >= 3
  eventually some receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse |
    replaceAppendEntriesConflictRequest[receiver, request, response]
} for 3 Node, 6 Term, 8 Message, 3 Index, 5 LogEntry, 2 Value

run appendEntriesResponseSuccessTrace {
  #Node = 3
  #Term >= 2
  eventually some leader: Node, response: AppendEntriesResponse |
    handleSuccessfulAppendEntriesResponse[leader, response]
    and some response.responseMatchIndex
} for 3 Node, 4 Term, 4 Message, 3 Index, 3 LogEntry, 2 Value

run appendEntriesResponseBackoffTrace {
  #Node = 3
  #Term >= 2
  eventually some leader: Node, response: AppendEntriesResponse |
    handleFailedAppendEntriesResponse[leader, response]
} for 3 Node, 4 Term, 6 Message, 3 Index, 3 LogEntry, 2 Value

run appendEntriesHigherTermResponseTrace {
  #Node = 3
  #Term >= 3
  eventually some receiver: Node, response: AppendEntriesResponse |
    higherTermAppendEntriesResponseStepDown[receiver, response]
} for 3 Node, 5 Term, 6 Message, 3 Index, 3 LogEntry, 2 Value
