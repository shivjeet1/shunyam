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
	cmd := exec.Command(a.binaryPath, "list-devices")

	// Stream output to logs or parse it
	_, err := cmd.Output()
	if err != nil {
		return err
	}

	return nil
}

// ShouldUseNativeProbe returns true if running on Windows or macOS
func ShouldUseNativeProbe() bool {
	return runtime.GOOS == "windows" || runtime.GOOS == "darwin"
}
