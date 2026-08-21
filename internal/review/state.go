package review

// State is where a proposal is in its short life.
type State string

// A proposal is open until it lands or is given up on. There is no state for
// "in review", because a proposal is always in review while it is open.
const (
	StateOpen      State = "open"
	StateLanded    State = "landed"
	StateAbandoned State = "abandoned"
)

// ParseState turns untrusted input into a State, and is the only door into one.
func ParseState(raw string) (State, error) {
	switch State(raw) {
	case StateOpen:
		return StateOpen, nil
	case StateLanded:
		return StateLanded, nil
	case StateAbandoned:
		return StateAbandoned, nil
	default:
		return "", ErrUnknownState
	}
}
