package policy

import (
	"strings"
	"testing"
	"time"
)

func TestNewGate(t *testing.T) {
	g := NewGate(true)
	if !g.requireLocalPresence {
		t.Error("expected requireLocalPresence to be true")
	}
	if g.challenges == nil {
		t.Error("expected challenges map to be initialized")
	}

	g2 := NewGate(false)
	if g2.requireLocalPresence {
		t.Error("expected requireLocalPresence to be false")
	}
}

func TestGenerateChallenge(t *testing.T) {
	g := NewGate(true)

	challenge, err := g.GenerateChallenge("/dev/sda")
	if err != nil {
		t.Fatalf("GenerateChallenge failed: %v", err)
	}

	// Should be 8 hex characters (4 bytes)
	if len(challenge) != 8 {
		t.Errorf("expected challenge length 8, got %d", len(challenge))
	}

	// Should be uppercase hex
	if challenge != strings.ToUpper(challenge) {
		t.Errorf("expected uppercase hex, got %s", challenge)
	}

	// Should be valid hex
	for _, c := range challenge {
		if !((c >= '0' && c <= '9') || (c >= 'A' && c <= 'F')) {
			t.Errorf("invalid hex character in challenge: %c", c)
		}
	}
}

func TestGenerateChallengeUnique(t *testing.T) {
	g := NewGate(true)

	seen := make(map[string]bool)
	for i := 0; i < 100; i++ {
		challenge, err := g.GenerateChallenge("/dev/sda")
		if err != nil {
			t.Fatalf("GenerateChallenge failed: %v", err)
		}
		if seen[challenge] {
			t.Errorf("duplicate challenge generated: %s", challenge)
		}
		seen[challenge] = true
	}
}

func TestCanWipeWithValidChallenge(t *testing.T) {
	g := NewGate(true)

	challenge, _ := g.GenerateChallenge("/dev/sda")
	err := g.CanWipe("/dev/sda", challenge)
	if err != nil {
		t.Errorf("expected nil error with valid challenge, got: %v", err)
	}
}

func TestCanWipeWithWrongChallenge(t *testing.T) {
	g := NewGate(true)

	g.GenerateChallenge("/dev/sda")
	err := g.CanWipe("/dev/sda", "WRONG123")
	if err != ErrUnauthorized {
		t.Errorf("expected ErrUnauthorized with wrong challenge, got: %v", err)
	}
}

func TestCanWipeWithoutChallenge(t *testing.T) {
	g := NewGate(true)

	err := g.CanWipe("/dev/sda", "ABCD1234")
	if err != ErrUnauthorized {
		t.Errorf("expected ErrUnauthorized without challenge, got: %v", err)
	}
}

func TestCanWipeChallengeOneTimeUse(t *testing.T) {
	g := NewGate(true)

	challenge, _ := g.GenerateChallenge("/dev/sda")

	// First use should succeed (if not mounted)
	_ = g.CanWipe("/dev/sda", challenge)

	// Second use should fail — challenge was consumed
	err := g.CanWipe("/dev/sda", challenge)
	if err != ErrUnauthorized {
		t.Errorf("expected ErrUnauthorized on reuse, got: %v", err)
	}
}

func TestCanWipeChallengeExpires(t *testing.T) {
	g := NewGate(true)

	challenge, _ := g.GenerateChallenge("/dev/sda")

	// Manually expire the challenge
	g.mu.Lock()
	g.challenges["/dev/sda"] = activeChallenge{
		expected: challenge,
		expires:  time.Now().Add(-1 * time.Second),
	}
	g.mu.Unlock()

	err := g.CanWipe("/dev/sda", challenge)
	if err != ErrUnauthorized {
		t.Errorf("expected ErrUnauthorized for expired challenge, got: %v", err)
	}
}

func TestCanWipeNoLocalPresence(t *testing.T) {
	g := NewGate(false)

	// Should skip challenge check entirely
	err := g.CanWipe("/dev/sda", "")
	if err != nil && err != ErrPartitionsMounted {
		// May fail on mount check, but should not be unauthorized
		if err == ErrUnauthorized {
			t.Error("should not require challenge when requireLocalPresence is false")
		}
	}
}

func TestCanWipeMountedDevice(t *testing.T) {
	g := NewGate(true)

	challenge, _ := g.GenerateChallenge("/dev/sda")

	// This test assumes /dev/sda is not mounted in the test environment
	// In a real CI environment, we'd need to mock /proc/mounts
	err := g.CanWipe("/dev/sda", challenge)
	if err == ErrPartitionsMounted {
		t.Skip("/dev/sda is mounted in this environment")
	}
}

func TestIsDeviceMountedNonExistent(t *testing.T) {
	// A device that doesn't exist in /proc/mounts should return false
	mounted, err := isDeviceMounted("/dev/nonexistent999")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if mounted {
		t.Error("expected mounted=false for non-existent device")
	}
}

func TestIsDeviceMountedInvalidPath(t *testing.T) {
	// A path that doesn't start with /dev/ should not match anything
	mounted, err := isDeviceMounted("/tmp/fakedevice")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if mounted {
		t.Error("expected mounted=false for /tmp path")
	}
}
