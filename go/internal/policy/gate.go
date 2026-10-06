package policy

import (
	"bufio"
	"crypto/rand"
	"encoding/hex"
	"errors"
	"fmt"
	"os"
	"strings"
	"sync"
	"time"
)

var (
	ErrActiveBootDrive   = errors.New("policy violation: target is the active boot drive")
	ErrPartitionsMounted = errors.New("policy violation: target has mounted partitions")
	ErrUnauthorized      = errors.New("policy violation: unauthorized caller (requires physical presence challenge)")
)

type activeChallenge struct {
	expected string
	expires  time.Time
}

// Gate evaluates if a device wipe operation is permitted and manages physical presence challenges
type Gate struct {
	requireLocalPresence bool
	challenges           map[string]activeChallenge // map[deviceID]challenge
	mu                   sync.Mutex
}

func NewGate(requireLocal bool) *Gate {
	return &Gate{
		requireLocalPresence: requireLocal,
		challenges:           make(map[string]activeChallenge),
	}
}

// GenerateChallenge creates a random 4-byte hex string (8 characters) for a specific device
func (g *Gate) GenerateChallenge(deviceID string) (string, error) {
	bytes := make([]byte, 4)
	if _, err := rand.Read(bytes); err != nil {
		return "", err
	}
	challenge := strings.ToUpper(hex.EncodeToString(bytes))

	g.mu.Lock()
	defer g.mu.Unlock()
	g.challenges[deviceID] = activeChallenge{
		expected: challenge,
		expires:  time.Now().Add(5 * time.Minute),
	}
	return challenge, nil
}

// CanWipe checks all policies before permitting a wipe on devicePath
func (g *Gate) CanWipe(devicePath string, challengeResponse string) error {
	// 1. Check Caller Authorization (Physical Presence Challenge)
	if g.requireLocalPresence {
		g.mu.Lock()
		challenge, exists := g.challenges[devicePath]
		if exists {
			delete(g.challenges, devicePath) // Challenge is one-time use
		}
		g.mu.Unlock()

		if !exists || time.Now().After(challenge.expires) || challengeResponse != challenge.expected {
			return ErrUnauthorized
		}
	}

	// 2. Prevent wiping of mounted drives (including boot drive)
	mounted, err := isDeviceMounted(devicePath)
	if err != nil {
		return err // fail secure if we can't determine mount state
	}
	if mounted {
		return ErrPartitionsMounted
	}

	return nil
}

// isDeviceMounted parses /proc/mounts to see if the device or its partitions are mounted.
func isDeviceMounted(devicePath string) (bool, error) {
	file, err := os.Open("/proc/mounts")
	if err != nil {
		return false, fmt.Errorf("cannot determine mount state: %w", err)
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := scanner.Text()
		fields := strings.Fields(line)
		if len(fields) > 0 {
			mountSource := fields[0]
			if strings.HasPrefix(mountSource, devicePath) {
				return true, nil
			}
		}
	}

	if err := scanner.Err(); err != nil {
		return false, err
	}

	return false, nil
}
