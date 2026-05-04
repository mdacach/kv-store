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
  some disj oldLeader, newLeader, emptyFollower: Node |
  some oldVoteReq, newVoteReq: RequestVoteRequest |
  some oldVoteResp, newVoteResp: RequestVoteResponse |
  some replicateReq, mismatchReq: AppendEntriesRequest |
  some replicateResp, mismatchResp: AppendEntriesResponse |
  some entry: LogEntry | {
    timeout[oldLeader]
    after sendRequestVoteRequest[oldLeader, newLeader, oldVoteReq]
    after after handleRequestVoteRequest[newLeader, oldVoteReq, oldVoteResp]
    after after after handleRequestVoteResponse[oldLeader, oldVoteResp]
    after after after after becomeLeader[oldLeader]
    after after after after after clientAppend[oldLeader, entry]
    after after after after after after sendAppendEntriesRequest[oldLeader, newLeader, replicateReq]
    after after after after after after after appendAppendEntriesNewEntryRequest[newLeader, replicateReq, replicateResp]
    after after after after after after after after timeout[newLeader]
    after after after after after after after after after sendRequestVoteRequest[newLeader, oldLeader, newVoteReq]
    after after after after after after after after after after handleRequestVoteRequest[oldLeader, newVoteReq, newVoteResp]
    after after after after after after after after after after after handleRequestVoteResponse[newLeader, newVoteResp]
    after after after after after after after after after after after after becomeLeader[newLeader]
    after after after after after after after after after after after after after sendAppendEntriesRequest[newLeader, emptyFollower, mismatchReq]
    after after after after after after after after after after after after after after rejectAppendEntriesPrevMismatch[emptyFollower, mismatchReq, mismatchResp]
  }
} for 16 steps, 3 Node, 4 Term, 8 Message, 2 Index, 1 LogEntry, 1 Value

run appendEntriesConflictRepairTrace {
  some disj oldLeader, newLeader, voter: Node |
  some oldReq, newReq: RequestVoteRequest |
  some oldResp, newResp: RequestVoteResponse |
  some appendReq: AppendEntriesRequest, appendResp: AppendEntriesResponse |
  some oldEntry, newEntry: LogEntry | {
    timeout[oldLeader]
    after sendRequestVoteRequest[oldLeader, voter, oldReq]
    after after handleRequestVoteRequest[voter, oldReq, oldResp]
    after after after handleRequestVoteResponse[oldLeader, oldResp]
    after after after after becomeLeader[oldLeader]
    after after after after after clientAppend[oldLeader, oldEntry]
    after after after after after after timeout[newLeader]
    after after after after after after after timeout[newLeader]
    after after after after after after after after sendRequestVoteRequest[newLeader, voter, newReq]
    after after after after after after after after after handleRequestVoteRequest[voter, newReq, newResp]
    after after after after after after after after after after handleRequestVoteResponse[newLeader, newResp]
    after after after after after after after after after after after becomeLeader[newLeader]
    after after after after after after after after after after after after clientAppend[newLeader, newEntry]
    after after after after after after after after after after after after after sendAppendEntriesRequest[newLeader, oldLeader, appendReq]
    after after after after after after after after after after after after after after replaceAppendEntriesConflictRequest[oldLeader, appendReq, appendResp]
  }
} for 16 steps, 3 Node, 4 Term, 6 Message, 1 Index, 2 LogEntry, 2 Value

run appendEntriesResponseSuccessTrace {
  #Node = 3
  #Term >= 2
  eventually some leader: Node, response: AppendEntriesResponse |
    handleSuccessfulAppendEntriesResponse[leader, response]
    and some response.responseMatchIndex
} for 3 Node, 4 Term, 4 Message, 3 Index, 3 LogEntry, 2 Value

run appendEntriesResponseBackoffTrace {
  some disj oldLeader, newLeader, emptyFollower: Node |
  some oldVoteReq, newVoteReq: RequestVoteRequest |
  some oldVoteResp, newVoteResp: RequestVoteResponse |
  some replicateReq, mismatchReq: AppendEntriesRequest |
  some replicateResp, mismatchResp: AppendEntriesResponse |
  some entry: LogEntry | {
    timeout[oldLeader]
    after sendRequestVoteRequest[oldLeader, newLeader, oldVoteReq]
    after after handleRequestVoteRequest[newLeader, oldVoteReq, oldVoteResp]
    after after after handleRequestVoteResponse[oldLeader, oldVoteResp]
    after after after after becomeLeader[oldLeader]
    after after after after after clientAppend[oldLeader, entry]
    after after after after after after sendAppendEntriesRequest[oldLeader, newLeader, replicateReq]
    after after after after after after after appendAppendEntriesNewEntryRequest[newLeader, replicateReq, replicateResp]
    after after after after after after after after timeout[newLeader]
    after after after after after after after after after sendRequestVoteRequest[newLeader, oldLeader, newVoteReq]
    after after after after after after after after after after handleRequestVoteRequest[oldLeader, newVoteReq, newVoteResp]
    after after after after after after after after after after after handleRequestVoteResponse[newLeader, newVoteResp]
    after after after after after after after after after after after after becomeLeader[newLeader]
    after after after after after after after after after after after after after sendAppendEntriesRequest[newLeader, emptyFollower, mismatchReq]
    after after after after after after after after after after after after after after rejectAppendEntriesPrevMismatch[emptyFollower, mismatchReq, mismatchResp]
    after after after after after after after after after after after after after after after handleFailedAppendEntriesResponse[newLeader, mismatchResp]
  }
} for 17 steps, 3 Node, 4 Term, 8 Message, 2 Index, 1 LogEntry, 1 Value

run appendEntriesHigherTermResponseTrace {
  #Node = 3
  #Term >= 3
  eventually some receiver: Node, response: AppendEntriesResponse |
    higherTermAppendEntriesResponseStepDown[receiver, response]
} for 3 Node, 5 Term, 6 Message, 3 Index, 3 LogEntry, 2 Value

run committedEntryTrace {
  #Node = 3
  #Term >= 2
  eventually some n: Node | some n.commitIndex
} for 15 steps, 3 Node, 4 Term, 8 Message, 3 Index, 3 LogEntry, 2 Value
