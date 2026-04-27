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
