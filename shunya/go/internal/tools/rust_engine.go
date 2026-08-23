package tools

import (
	"os/exec"
	"runtime"
)

// RustEngineAdapter calls the Rust worker process
type RustEngineAdapter struct {
	binaryPath string
}

func NewRustEngineAdapter(binaryPath string) *RustEngineAdapter {
	if binaryPath == "" {
		binaryPath = "shunya-engine"
	}
	return &RustEngineAdapter{binaryPath: binaryPath}
}

// ListDevices triggers the Rust ioctl probing (intended for Windows/macOS).
func (a *RustEngineAdapter) ListDevices() error {
	// For MVP, we just trigger it and check if it succeeds.
	// In the real system, this will communicate over stdin/stdout using gRPC protobufs.
	cmd := exec.Command(a.binaryPath, "list-devices")
	
	// Stream output to logs or parse it
	_, err := cmd.Output()
	if err != nil {
		return err
	}
	
	return nil
}

// ShouldUseNativeProbe returns true if we are on Windows or macOS
func ShouldUseNativeProbe() bool {
	return runtime.GOOS == "windows" || runtime.GOOS == "darwin"
}
