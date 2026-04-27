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
  DropStaleResponseEvent,
  DropMessageEvent,
  DuplicateMessageEvent,
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

fun drop_stale_response_happens : Event -> Node {
  { e : DropStaleResponseEvent, n : Node |
    some resp : Message | dropStaleResponse[n, resp]
  }
}

fun drop_message_happens : set Event {
  { e : DropMessageEvent |
    some message : Message | dropMessage[message]
  }
}

fun duplicate_message_happens : set Event {
  { e : DuplicateMessageEvent |
    some message, duplicate : Message | duplicateMessage[message, duplicate]
  }
}

fun client_append_happens : Event -> Node {
  { e : ClientAppendEvent, n : Node |
    some entry : Entry | clientAppend[n, entry]
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
  { e : HandleAppendEntriesResponseEvent, l, s : Node |
    some resp : AppendEntriesResponse |
      resp.source = s and handleAppendEntriesResponse[l, resp]
  }
}

fun advance_commit_index_happens : Event -> Node {
  { e : AdvanceCommitIndexEvent, n : Node |
    some i : Index | advanceCommitIndex[n, i]
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
  drop_stale_response_happens.Node +
  drop_message_happens +
  duplicate_message_happens +
  client_append_happens.Node +
  send_append_entries_request_happens.Node.Node +
  handle_append_entries_request_happens.Node.Node +
  handle_append_entries_response_happens.Node.Node +
  advance_commit_index_happens.Node +
  stutter_happens
}
