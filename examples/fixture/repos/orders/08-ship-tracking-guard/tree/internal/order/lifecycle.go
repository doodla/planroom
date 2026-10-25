package order

import (
	"errors"
	"fmt"
	"time"
)

// Event is something that happens to an order.
type Event string

const (
	EventPay         Event = "pay"
	EventPaymentFail Event = "payment_fail"
	EventCancel      Event = "cancel"
	EventFulfil      Event = "fulfil"
	EventShip        Event = "ship"
	EventDeliver     Event = "deliver"
	EventRelease     Event = "release"
)

var (
	// ErrInvalidTransition means the event is not allowed in the order's current state.
	ErrInvalidTransition = errors.New("order: invalid transition")
	// ErrGuardFailed means the transition exists but its guard rejected it.
	ErrGuardFailed = errors.New("order: guard failed")
)

// Guard decides whether a transition may happen.
type Guard func(o *Order) bool

type transition struct {
	From  State
	Event Event
	To    State
	Guard Guard
}

// transitions is the whole order lifecycle. Anything not listed is rejected.
// docs/order-lifecycle.md describes it for humans.
var transitions = []transition{
	{From: StateCreated, Event: EventPay, To: StatePaid},
	{From: StateCreated, Event: EventPaymentFail, To: StatePaymentFailed},
	{From: StateCreated, Event: EventCancel, To: StateCancelled},
	{From: StatePaid, Event: EventFulfil, To: StateFulfilling},
	{From: StatePaid, Event: EventCancel, To: StateCancelled},
	{From: StateFulfilling, Event: EventShip, To: StateShipped, Guard: hasTrackingNumber},
	{From: StateShipped, Event: EventDeliver, To: StateDelivered},
	{From: StateOnHold, Event: EventRelease, To: StatePaid},
}

// hasTrackingNumber guards ship: a parcel cannot leave without one.
func hasTrackingNumber(o *Order) bool {
	return o.TrackingNumber != ""
}

// IsTerminal reports whether no further events apply to an order in s.
func IsTerminal(s State) bool {
	return s == StateDelivered || s == StateCancelled
}

// Apply moves o through ev and returns the state it left.
func Apply(o *Order, ev Event) (State, error) {
	for _, t := range transitions {
		if t.From != o.State || t.Event != ev {
			continue
		}
		if t.Guard != nil && !t.Guard(o) {
			return o.State, fmt.Errorf("%w: %s from %s", ErrGuardFailed, ev, o.State)
		}
		from := o.State
		o.State = t.To
		o.UpdatedAt = time.Now().UTC()
		return from, nil
	}
	return o.State, fmt.Errorf("%w: %s from %s", ErrInvalidTransition, ev, o.State)
}
