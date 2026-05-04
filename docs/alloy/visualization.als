module visualization
open raft

// Visualizer-only event tags.
// These are not part of the protocol state; they exist so the Alloy visualizer
// can show which transition fired in a given step.
enum Event {
  TimeoutEvent,
  SendRequestVoteRequestEvent,
  HandleRequestVoteRequestEvent,
  HandleRequestVoteResponseEvent,
  BecomeLeaderEvent,
  ClientAppendEvent,
  SendAppendEntriesRequestEvent,
  HandleAppendEntriesRequestEvent,
  HandleAppendEntriesResponseEvent,
  AdvanceCommitIndexEvent,
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

// Direct network edges for in-flight AppendEntries requests.
fun inFlightAppendEntriesRequestEdges : Node -> Node {
  { s, d : Node |
    some req : AppendEntriesRequest & InFlight |
      req.source = s and req.dest = d
  }
}

// Direct network edges for in-flight successful AppendEntries responses.
fun inFlightAppendEntriesSuccessResponseEdges : Node -> Node {
  { s, d : Node |
    some resp : AppendEntriesResponse & InFlight |
      resp.source = s and resp.dest = d and resp.appendSuccess = True
  }
}

// Direct network edges for in-flight failed AppendEntries responses.
fun inFlightAppendEntriesFailureResponseEdges : Node -> Node {
  { s, d : Node |
    some resp : AppendEntriesResponse & InFlight |
      resp.source = s and resp.dest = d and resp.appendSuccess = False
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

fun become_leader_happens : Event -> Node {
  { e : BecomeLeaderEvent, n : Node | becomeLeader[n] }
}

fun client_append_happens : Event -> Node {
  { e : ClientAppendEvent, n : Node |
    some entry : LogEntry | clientAppend[n, entry]
  }
}

fun send_append_entries_request_happens : Event -> Node -> Node {
  { e : SendAppendEntriesRequestEvent, l, o : Node |
    some req : AppendEntriesRequest | sendAppendEntriesRequest[l, o, req]
  }
}

fun handle_append_entries_request_happens : Event -> Node -> Node {
  { e : HandleAppendEntriesRequestEvent, r, s : Node |
    some req : AppendEntriesRequest, resp : AppendEntriesResponse |
      req.source = s and handleAppendEntriesRequest[r, req, resp]
  }
}

fun handle_append_entries_response_happens : Event -> Node -> Node {
  { e : HandleAppendEntriesResponseEvent, r, s : Node |
    some resp : AppendEntriesResponse |
      resp.source = s and handleAppendEntriesResponse[r, resp]
  }
}

fun advance_commit_index_happens : Event -> Node -> Index {
  { e : AdvanceCommitIndexEvent, l : Node, i : Index |
    advanceCommitIndex[l, i]
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
  become_leader_happens.Node +
  client_append_happens.Node +
  send_append_entries_request_happens.Node.Node +
  handle_append_entries_request_happens.Node.Node +
  handle_append_entries_response_happens.Node.Node +
  advance_commit_index_happens.Node.Index +
  stutter_happens
}
