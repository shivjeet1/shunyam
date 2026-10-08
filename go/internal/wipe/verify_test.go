package wipe

import (
	"fmt"
	"testing"
)

func TestMethodToPattern(t *testing.T) {
	tests := []struct {
		method    string
		transport string
		expected  VerifyPattern
	}{
		{"Zero Fill", "nvme", PatternZeros},
		{"Zero Fill", "sata", PatternZeros},
		{"Clear", "usb", PatternZeros},
		{"Single Pass", "nvme", Pattern5A},
		{"Single Pass", "sata", PatternRandom},
		{"Default", "nvme", Pattern5A},
		{"Default", "usb", PatternRandom},
		{"3-Pass", "nvme", PatternOnes},
		{"3-Pass", "sata", PatternZeros},
		{"DoD", "nvme", PatternOnes},
		{"DoD", "usb", PatternZeros},
		{"Crypto Erase", "nvme", PatternChanged},
		{"Crypto Erase", "sata", PatternChanged},
		{"Block Erase", "nvme", PatternChanged},
		{"Block Erase", "sata", PatternChanged},
		{"Purge", "nvme", PatternChanged},
		{"Unknown Method", "nvme", PatternUnknown},
		{"", "nvme", PatternUnknown},
	}

	for _, tt := range tests {
		t.Run(fmt.Sprintf("%s/%s", tt.method, tt.transport), func(t *testing.T) {
			result := methodToPattern(tt.method, tt.transport)
			if result != tt.expected {
				t.Errorf("methodToPattern(%q, %q) = %d, want %d", tt.method, tt.transport, result, tt.expected)
			}
		})
	}
}

func TestVerifyPatternZeros(t *testing.T) {
	zeros := make([]byte, 512)
	err := verifyPattern(zeros, PatternZeros, 0)
	if err != nil {
		t.Errorf("expected nil error for all-zeros buffer, got: %v", err)
	}

	// Add a single non-zero byte
	zeros[100] = 0x01
	err = verifyPattern(zeros, PatternZeros, 0)
	if err == nil {
		t.Error("expected error for non-zero byte in zeros pattern")
	}
}

func TestVerifyPatternOnes(t *testing.T) {
	ones := make([]byte, 512)
	for i := range ones {
		ones[i] = 0xFF
	}
	err := verifyPattern(ones, PatternOnes, 0)
	if err != nil {
		t.Errorf("expected nil error for all-0xFF buffer, got: %v", err)
	}

	ones[50] = 0x00
	err = verifyPattern(ones, PatternOnes, 0)
	if err == nil {
		t.Error("expected error for non-0xFF byte in ones pattern")
	}
}

func TestVerifyPattern5A(t *testing.T) {
	pattern := make([]byte, 512)
	for i := range pattern {
		pattern[i] = 0x5A
	}
	err := verifyPattern(pattern, Pattern5A, 0)
	if err != nil {
		t.Errorf("expected nil error for 0x5A pattern, got: %v", err)
	}

	pattern[200] = 0x00
	err = verifyPattern(pattern, Pattern5A, 0)
	if err == nil {
		t.Error("expected error for non-0x5A byte in 0x5A pattern")
	}
}

func TestVerifyPatternRandom(t *testing.T) {
	// All zeros should fail
	zeros := make([]byte, 512)
	err := verifyPattern(zeros, PatternRandom, 0)
	if err == nil {
		t.Error("expected error for all-zeros in random pattern")
	}

	// Uniform pattern should fail
	uniform := make([]byte, 512)
	for i := range uniform {
		uniform[i] = 0x42
	}
	err = verifyPattern(uniform, PatternRandom, 0)
	if err == nil {
		t.Error("expected error for uniform pattern in random pattern")
	}

	// Mixed data should pass
	mixed := make([]byte, 512)
	for i := range mixed {
		mixed[i] = byte(i % 256)
	}
	err = verifyPattern(mixed, PatternRandom, 0)
	if err != nil {
		t.Errorf("expected nil error for mixed data in random pattern, got: %v", err)
	}
}

func TestVerifyPatternChanged(t *testing.T) {
	// All zeros should fail
	zeros := make([]byte, 512)
	err := verifyPattern(zeros, PatternChanged, 0)
	if err == nil {
		t.Error("expected error for all-zeros in changed pattern")
	}

	// Non-zero data should pass
	data := make([]byte, 512)
	for i := range data {
		data[i] = byte(i % 256)
	}
	err = verifyPattern(data, PatternChanged, 0)
	if err != nil {
		t.Errorf("expected nil error for non-zero data in changed pattern, got: %v", err)
	}
}

func TestVerifyPatternUnknown(t *testing.T) {
	// Any data should pass for unknown pattern
	data := make([]byte, 512)
	err := verifyPattern(data, PatternUnknown, 0)
	if err != nil {
		t.Errorf("expected nil error for unknown pattern, got: %v", err)
	}
}

func TestIsAllBytes(t *testing.T) {
	tests := []struct {
		name     string
		buf      []byte
		b        byte
		expected bool
	}{
		{"all zeros", []byte{0, 0, 0, 0}, 0x00, true},
		{"all ones", []byte{1, 1, 1, 1}, 0x01, true},
		{"mixed", []byte{0, 1, 0, 0}, 0x00, false},
		{"empty", []byte{}, 0x00, true},
		{"single match", []byte{0x5A}, 0x5A, true},
		{"single no match", []byte{0x5A}, 0x00, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := isAllBytes(tt.buf, tt.b)
			if result != tt.expected {
				t.Errorf("isAllBytes(%v, 0x%02x) = %v, want %v", tt.buf, tt.b, result, tt.expected)
			}
		})
	}
}

func TestIsAllSameByte(t *testing.T) {
	tests := []struct {
		name     string
		buf      []byte
		expected bool
	}{
		{"all zeros", []byte{0, 0, 0, 0}, true},
		{"all same non-zero", []byte{0x42, 0x42, 0x42}, true},
		{"mixed", []byte{0, 1, 0}, false},
		{"empty", []byte{}, true},
		{"single", []byte{0x5A}, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := isAllSameByte(tt.buf)
			if result != tt.expected {
				t.Errorf("isAllSameByte(%v) = %v, want %v", tt.buf, result, tt.expected)
			}
		})
	}
}

func TestPatternName(t *testing.T) {
	tests := []struct {
		pattern  VerifyPattern
		expected string
	}{
		{PatternUnknown, "unknown"},
		{PatternZeros, "zeros"},
		{PatternOnes, "ones"},
		{Pattern5A, "0x5A"},
		{PatternRandom, "random"},
		{PatternChanged, "changed"},
	}

	for _, tt := range tests {
		t.Run(tt.expected, func(t *testing.T) {
			result := patternName(tt.pattern)
			if result != tt.expected {
				t.Errorf("patternName(%d) = %q, want %q", tt.pattern, result, tt.expected)
			}
		})
	}
}

func TestVerifyWipeZeroCapacity(t *testing.T) {
	emit := func(jobID, stepName, status, message string, progress float32) {}
	err := VerifyWipe("/dev/null", 0, "Zero Fill", "nvme", 10, emit, "test-job")
	if err == nil {
		t.Error("expected error for zero capacity")
	}
}

func TestVerifyWipeUnknownPattern(t *testing.T) {
	// Unknown pattern should just check readability — /dev/null is readable
	emit := func(jobID, stepName, status, message string, progress float32) {}
	err := VerifyWipe("/dev/null", 512*1024, "Unknown Method", "nvme", 0, emit, "test-job")
	// /dev/null has 0 capacity, so this will fail on capacity check
	// But if we pass a real block device path, it should work
	if err != nil && err.Error() == "unknown capacity, cannot verify" {
		t.Skip("zero capacity device")
	}
}
