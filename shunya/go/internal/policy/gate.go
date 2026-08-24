package policy

import (
	"bufio"
	"errors"
	"os"
	"strings"
)

var (
	ErrActiveBootDrive   = errors.New("policy violation: target is the active boot drive")
	ErrPartitionsMounted = errors.New("policy violation: target has mounted partitions")
	ErrUnauthorized      = errors.New("policy violation: unauthorized caller (requires physical presence or signed token)")
)

// Gate evaluates if a device wipe operation is permitted
type Gate struct {
	requireLocalPresence bool
}

func NewGate(requireLocal bool) *Gate {
	return &Gate{
		requireLocalPresence: requireLocal,
	}
}

// CanWipe checks all policies before permitting a wipe on devicePath (e.g. "/dev/nvme0n1")
func (g *Gate) CanWipe(devicePath string, isAuthorized bool) error {
	// 1. Check Caller Authorization
	if g.requireLocalPresence && !isAuthorized {
		return ErrUnauthorized
	}

	// 2. Prevent wiping of mounted drives (including boot drive)
	mounted, err := isDeviceMounted(devicePath)
	if err != nil {
		return err // fail secure if we can't determine mount state
	}
	if mounted {
		// As a heuristic for MVP, if it's mounted, we assume it could be the boot drive or a protected mount
		return ErrPartitionsMounted
	}

	return nil
}

// isDeviceMounted parses /proc/mounts to see if the device or its partitions are mounted.
func isDeviceMounted(devicePath string) (bool, error) {
	file, err := os.Open("/proc/mounts")
	if err != nil {
		// If we are on Windows/macOS, we need OS-specific checks.
		// For MVP, if /proc/mounts doesn't exist, we assume we aren't on Linux or can't read it.
		if os.IsNotExist(err) {
			return false, nil // Bypass for non-Linux testing
		}
		return false, err
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := scanner.Text()
		fields := strings.Fields(line)
		if len(fields) > 0 {
			mountSource := fields[0]
			// e.g., if devicePath is "/dev/nvme0n1", and mountSource is "/dev/nvme0n1p1"
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
