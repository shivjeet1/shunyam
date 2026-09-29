package tools

import (
	"os"
	"os/exec"
	"path/filepath"
)

func FindEngineBinary() string {
	if p := os.Getenv("SHUNYA_ENGINE_PATH"); p != "" {
		return p
	}
	if exe, err := os.Executable(); err == nil {
		candidate := filepath.Join(filepath.Dir(exe), "shunya-engine")
		if _, err := os.Stat(candidate); err == nil {
			return candidate
		}
	}
	if _, err := os.Stat("./build/shunya-engine"); err == nil {
		return "./build/shunya-engine"
	}
	if path, err := exec.LookPath("shunya-engine"); err == nil {
		return path
	}
	return "shunya-engine"
}
