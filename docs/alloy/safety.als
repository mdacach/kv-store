module safety
open raft
open util/ordering[Index] as indexOrd

// Safety property: every node should always be in exactly one Raft role.
assert RolePartition {
  always {
    Node = Follower + Candidate + Leader
    disj[Follower, Candidate, Leader]
  }
}

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

// Safety: handling a higher-term vote request uses the generic step-down path
// and forces the receiver out of candidate/leader state and back to follower.
assert HigherTermRequestForcesStepDown {
  always all receiver: Node, request: RequestVoteRequest, response: RequestVoteResponse |
    (handleRequestVoteRequest[receiver, request, response]
      and termGt[request.messageTerm, receiver.currentTerm]) implies
        (receiver in Follower'
         and receiver not in Candidate'
         and receiver not in Leader')
}

// Safety: dropping a stale response only consumes that response from the
// network.
assert DropStaleResponseOnlyConsumesNetwork {
  always all receiver: Node, response: Message |
    dropStaleResponse[receiver, response] implies {
      Follower' = Follower
      Candidate' = Candidate
      Leader' = Leader
      currentTerm' = currentTerm
      votedFor' = votedFor
      votesGranted' = votesGranted
      votesResponded' = votesResponded
      log' = log
      nextIndex' = nextIndex
      matchIndex' = matchIndex
    }
}

// Safety: dropping a message only removes it from the network.
assert DropMessageOnlyConsumesNetwork {
  always all message: Message |
    dropMessage[message] implies {
      Follower' = Follower
      Candidate' = Candidate
      Leader' = Leader
      currentTerm' = currentTerm
      votedFor' = votedFor
      votesGranted' = votesGranted
      votesResponded' = votesResponded
      log' = log
      nextIndex' = nextIndex
      matchIndex' = matchIndex
      commitIndex' = commitIndex
    }
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

// Safety: granted votes are a subset of the responses the candidate has
// recorded. Self-votes count as responded in this model.
assert VotesGrantedSubsetVotesResponded {
  always all n: Node | n.votesGranted in n.votesResponded
}

// Safety: leaders never record a follower match index beyond the leader's own
// log.
assert LeaderMatchIndexWithinLog {
  always all leader: Leader, peer: Node |
    leader.matchIndex[peer] in logIndexes[leader]
}

// Safety: commit indexes always refer to entries in the node's log.
assert CommitIndexWithinLog {
  always all n: Node | n.commitIndex in logIndexes[n]
}

// Safety: commit indexes do not move backward.
assert CommitIndexMonotonic {
  always all n: Node | commitDoesNotMoveBackward[n]
}

// Safety: handling a failed AppendEntries response never moves nextIndex below
// the first bounded index.
assert FailedAppendEntriesKeepsNextIndexInScope {
  always all leader: Node, response: AppendEntriesResponse |
    (handleAppendEntriesResponse[leader, response]
      and response.appendSuccess = False) implies
        leader.nextIndex'[response.source] in Index
}

// Safety: a node that remains leader in the same term never changes entries
// already present in its own log.
assert LeaderAppendOnly {
  always all n: Node, i: logIndexes[n] |
    (n in Leader and n in Leader' and n.currentTerm' = n.currentTerm) implies
      i.(n.log') = i.(n.log)
}

// Safety: if two logs contain entries with the same index and term, then the
// entries at that index and all preceding entries are identical.
assert LogMatching {
  always all n1, n2: Node, i: Index |
    (
      some logEntry[n1, i]
      and logEntry[n1, i].entryTerm = logEntry[n2, i].entryTerm
    ) implies {
      logEntry[n1, i] = logEntry[n2, i]
      all earlier: Index |
        i in earlier.^(indexOrd/next) implies
          logEntry[n1, earlier] = logEntry[n2, earlier]
    }
}

// Safety: leaders in later terms contain entries committed in earlier terms.
assert LeaderCompleteness {
  always all leader: Leader, n: Node, i: Index |
    (
      committedThrough[n, i]
      and some logEntry[n, i]
      and termGt[leader.currentTerm, logEntry[n, i].entryTerm]
    ) implies
      logEntry[leader, i] = logEntry[n, i]
}

// Safety: no two nodes can have different committed entries at the same index.
assert CommittedEntryAgreement {
  always all n1, n2: Node, i: Index |
    (
      committedThrough[n1, i]
      and committedThrough[n2, i]
    ) implies
      logEntry[n1, i] = logEntry[n2, i]
}

// Safety: every in-flight AppendEntries request carries previous-log metadata
// that agrees with its source log.
assert AppendEntriesPrevLogMatchesSource {
  always all request: AppendEntriesRequest & InFlight |
    (
      no request.prevLogIndex
      and no request.prevLogTerm
    ) or (
      request.prevLogIndex in logIndexes[request.source]
      and request.prevLogTerm = logEntry[request.source, request.prevLogIndex].entryTerm
    )
}

// Safety: successful AppendEntries handling only succeeds when the previous-log
// metadata matched the receiver log before the request was applied.
assert SuccessfulAppendEntriesRequiresPrevLogMatch {
  always all receiver: Node, request: AppendEntriesRequest, response: AppendEntriesResponse |
    (handleAppendEntriesRequest[receiver, request, response]
      and response.appendSuccess = True) implies
        prevLogMatches[receiver, request]
}

// Safety: any granted vote is only granted to a candidate whose log metadata is
// at least as up-to-date as the receiver's log.
assert GrantedVotesRequireUpToDateLog {
  always all receiver: Node, request: RequestVoteRequest, response: RequestVoteResponse |
    (handleRequestVoteRequest[receiver, request, response]
      and response.voteGranted = True) implies
        logUpToDate[request.requestLastLogIndex, request.requestLastLogTerm, receiver]
}

// Safety: each node has at most one log entry at each log index.
assert OneEntryPerNodeIndex {
  always all n: Node, i: Index | lone logEntry[n, i]
}

// Safety: occupied log indexes form a contiguous prefix.
assert LogsAreContiguous {
  always all n: Node, i: logIndexes[n], earlier: Index |
    i in earlier.^(indexOrd/next) implies earlier in logIndexes[n]
}

check RolePartition for 5 Node, 6 Term, 4 Message
check LeadersRequireMajority for 5 Node, 6 Term, 4 Message
check LeadersKeepTheirElectionTerm for 5 Node, 6 Term, 4 Message
check LeadersStepDownBeforeTermChange for 5 Node, 6 Term, 4 Message
check HigherTermRequestForcesStepDown for 5 Node, 6 Term, 4 Message
check DropStaleResponseOnlyConsumesNetwork for 5 Node, 6 Term, 5 Message, 4 Index, 4 Entry, 2 Value
check DropMessageOnlyConsumesNetwork for 5 Node, 6 Term, 5 Message, 4 Index, 4 Entry, 2 Value
check OneVotePerNodePerTerm for 5 Node, 6 Term, 4 Message
check AtMostOneLeaderPerTerm for 5 Node, 6 Term, 4 Message
check VotesGrantedSubsetVotesResponded for 5 Node, 6 Term, 4 Message
check LeaderMatchIndexWithinLog for 5 Node, 6 Term, 4 Message, 4 Index, 4 Entry, 2 Value
check CommitIndexWithinLog for 5 Node, 6 Term, 5 Message, 4 Index, 4 Entry, 2 Value
check CommitIndexMonotonic for 5 Node, 6 Term, 5 Message, 4 Index, 4 Entry, 2 Value
check FailedAppendEntriesKeepsNextIndexInScope for 5 Node, 6 Term, 5 Message, 4 Index, 4 Entry, 2 Value
check LeaderAppendOnly for 5 Node, 6 Term, 4 Message, 4 Index, 4 Entry, 2 Value
check LogMatching for 5 Node, 6 Term, 5 Message, 4 Index, 4 Entry, 2 Value
check LeaderCompleteness for 5 Node, 6 Term, 5 Message, 4 Index, 4 Entry, 2 Value
check CommittedEntryAgreement for 5 Node, 6 Term, 5 Message, 4 Index, 4 Entry, 2 Value
check AppendEntriesPrevLogMatchesSource for 5 Node, 6 Term, 5 Message, 4 Index, 4 Entry, 2 Value
check SuccessfulAppendEntriesRequiresPrevLogMatch for 5 Node, 6 Term, 5 Message, 4 Index, 4 Entry, 2 Value
check GrantedVotesRequireUpToDateLog for 5 Node, 6 Term, 4 Message, 4 Index, 4 Entry, 2 Value
check OneEntryPerNodeIndex for 5 Node, 6 Term, 4 Message, 4 Index, 4 Entry, 2 Value
check LogsAreContiguous for 5 Node, 6 Term, 4 Message, 4 Index, 4 Entry, 2 Value
