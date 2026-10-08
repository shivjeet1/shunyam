package wipe

import (
	"encoding/json"
	"fmt"
	"math/rand"
	"os"
	"os/exec"
	"strings"
	"time"
)

// VerifyPattern defines what data pattern to expect after a wipe
type VerifyPattern int

const (
	PatternUnknown VerifyPattern = iota
	PatternZeros
	PatternOnes
	Pattern5A
	PatternRandom
	PatternChanged
)

// methodToPattern determines the expected data pattern based on wipe method and transport
func methodToPattern(method string, transport string) VerifyPattern {
	methodLower := strings.ToLower(method)

	if strings.Contains(methodLower, "zero fill") || strings.Contains(methodLower, "clear") {
		return PatternZeros
	}
	if strings.Contains(methodLower, "single pass") || strings.Contains(methodLower, "default") {
		if transport == "nvme" {
			return Pattern5A
		}
		return PatternRandom
	}
	if strings.Contains(methodLower, "3-pass") || strings.Contains(methodLower, "dod") {
		if transport == "nvme" {
			return PatternOnes
		}
		return PatternZeros
	}
	if strings.Contains(methodLower, "crypto erase") || strings.Contains(methodLower, "block erase") || strings.Contains(methodLower, "purge") {
		return PatternChanged
	}
	return PatternUnknown
}

// VerifyWipe performs post-wipe verification by reading sample sectors and
// checking they contain the expected data pattern for the given wipe method.
func VerifyWipe(devicePath string, capacityBytes uint64, method string, transport string, numSamples int, emit EmitFunc, jobID string) error {
	if capacityBytes == 0 {
		return fmt.Errorf("unknown capacity, cannot verify")
	}

	pattern := methodToPattern(method, transport)

	// For NVMe sanitize methods, poll sanitize status to confirm completion
	if transport == "nvme" && pattern != PatternUnknown {
		ctrl := controllerFromNamespace(devicePath)
		if err := verifySanitizeComplete(ctrl, emit, jobID); err != nil {
			return fmt.Errorf("sanitize verification failed: %w", err)
		}
	}

	totalSectors := capacityBytes / 512

	f, err := os.Open(devicePath)
	if err != nil {
		return err
	}
	defer f.Close()

	// Sample sectors: start, middle, end, plus random samples
	samples := []uint64{0, totalSectors / 2, totalSectors - 1}
	for i := 0; i < numSamples; i++ {
		samples = append(samples, uint64(rand.Int63n(int64(totalSectors))))
	}

	for i, s := range samples {
		buf := make([]byte, 512)
		_, err := f.ReadAt(buf, int64(s*512))
		if err != nil {
			return fmt.Errorf("failed to read sector %d: %w", s, err)
		}

		if err := verifyPattern(buf, pattern, s); err != nil {
			return err
		}

		emit(jobID, "Verifying", "InProgress",
			fmt.Sprintf("Verified %d/%d samples (pattern: %s)", i+1, len(samples), patternName(pattern)),
			float32(i+1)*100.0/float32(len(samples)))
	}

	return nil
}

func verifyPattern(buf []byte, pattern VerifyPattern, sector uint64) error {
	switch pattern {
	case PatternZeros:
		if !isAllBytes(buf, 0x00) {
			return fmt.Errorf("sector %d: expected all zeros, got mixed data", sector)
		}
	case PatternOnes:
		if !isAllBytes(buf, 0xFF) {
			return fmt.Errorf("sector %d: expected all 0xFF, got mixed data", sector)
		}
	case Pattern5A:
		if !isAllBytes(buf, 0x5A) {
			return fmt.Errorf("sector %d: expected 0x5A pattern, got mixed data", sector)
		}
	case PatternRandom:
		if isAllBytes(buf, 0x00) {
			return fmt.Errorf("sector %d: expected non-zero data, got all zeros", sector)
		}
		if isAllSameByte(buf) {
			return fmt.Errorf("sector %d: expected random data, got uniform pattern", sector)
		}
	case PatternChanged:
		if isAllBytes(buf, 0x00) {
			return fmt.Errorf("sector %d: expected non-zero data after erase, got all zeros", sector)
		}
	case PatternUnknown:
		// Can't verify pattern, just check readability (already done by ReadAt)
	}
	return nil
}

func isAllBytes(buf []byte, b byte) bool {
	for _, v := range buf {
		if v != b {
			return false
		}
	}
	return true
}

func isAllSameByte(buf []byte) bool {
	if len(buf) == 0 {
		return true
	}
	first := buf[0]
	for _, v := range buf[1:] {
		if v != first {
			return false
		}
	}
	return true
}

func patternName(p VerifyPattern) string {
	switch p {
	case PatternZeros:
		return "zeros"
	case PatternOnes:
		return "ones"
	case Pattern5A:
		return "0x5A"
	case PatternRandom:
		return "random"
	case PatternChanged:
		return "changed"
	default:
		return "unknown"
	}
}

// verifySanitizeComplete polls nvme sanitize-log until the sanitize operation
// completes successfully or fails.
func verifySanitizeComplete(controllerDev string, emit EmitFunc, jobID string) error {
	for {
		cmd := exec.Command("nvme", "sanitize-log", controllerDev, "-o", "json")
		out, err := cmd.Output()
		if err != nil {
			return fmt.Errorf("nvme sanitize-log failed: %w", err)
		}

		var log struct {
			Sprog uint16 `json:"sprog"`
			Sstat uint16 `json:"sstat"`
		}
		if err := json.Unmarshal(out, &log); err != nil {
			return fmt.Errorf("failed to parse sanitize-log: %w", err)
		}

		status := log.Sstat & 0x7
		progress := float32(log.Sprog) * 100.0 / 65536.0

		emit(jobID, "Verifying", "InProgress",
			fmt.Sprintf("Sanitize status: %d, progress: %.1f%%", status, progress), progress)

		if status == 0x1 {
			return nil // completed successfully
		}
		if status == 0x3 {
			return fmt.Errorf("nvme sanitize failed (sstat=3)")
		}

		time.Sleep(2 * time.Second)
	}
}
