package order

import (
	"errors"
	"testing"
)

func TestApplyHappyPath(t *testing.T) {
	o := New("cus_1", "USD", []Item{{SKU: "mug", Quantity: 2, PriceCents: 1200}})
	o.TrackingNumber = "1Z999AA10123456784"
	for _, ev := range []Event{EventPay, EventFulfil, EventShip, EventDeliver} {
		if _, err := Apply(o, ev); err != nil {
			t.Fatalf("%s: %v", ev, err)
		}
	}
	if o.State != StateDelivered || !IsTerminal(o.State) {
		t.Fatalf("state = %s, want delivered", o.State)
	}
}

func TestShipNeedsTrackingNumber(t *testing.T) {
	o := &Order{State: StateFulfilling}
	_, err := Apply(o, EventShip)
	if !errors.Is(err, ErrGuardFailed) {
		t.Fatalf("err = %v, want ErrGuardFailed", err)
	}
	if o.State != StateFulfilling {
		t.Fatalf("state changed to %s", o.State)
	}
}

func TestRejectsUnknownTransition(t *testing.T) {
	o := &Order{State: StateShipped}
	if _, err := Apply(o, EventCancel); !errors.Is(err, ErrInvalidTransition) {
		t.Fatalf("err = %v, want ErrInvalidTransition", err)
	}
}
