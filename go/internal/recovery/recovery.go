package recovery

import (
	"bufio"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

type RecoveryConfig struct {
	DevicePath string
	OutputDir  string
	Profile    string
	JobID      string
}

type EmitFunc func(jobID, status, message string, progress float32)

func Execute(cfg RecoveryConfig, emit EmitFunc) error {
	if _, err := exec.LookPath("photorec"); err != nil {
		return fmt.Errorf("photorec is not installed. Install testdisk package.")
	}

	if err := os.MkdirAll(cfg.OutputDir, 0755); err != nil {
		return err
	}

	cmd := exec.Command("photorec", "/log", "/d", cfg.OutputDir, "/cmd", cfg.DevicePath, "search")
	stderr, err := cmd.StderrPipe()
	if err != nil {
		return err
	}

	if err := cmd.Start(); err != nil {
		return err
	}

	emit(cfg.JobID, "InProgress", "Recovery started", 0)

	done := make(chan struct{})
	go func() {
		scanner := bufio.NewScanner(stderr)
		for scanner.Scan() {
			line := scanner.Text()
			if strings.Contains(line, "%") {
				emit(cfg.JobID, "InProgress", "Recovering...", 50)
			}
		}
		close(done)
	}()

	ticker := time.NewTicker(2 * time.Second)
	defer ticker.Stop()

	go func() {
		for {
			select {
			case <-done:
				return
			case <-ticker.C:
				countFiles(cfg.OutputDir)
			}
		}
	}()

	err = cmd.Wait()
	total := countFiles(cfg.OutputDir)

	if err != nil {
		return fmt.Errorf("photorec failed: %w", err)
	}

	emit(cfg.JobID, "Success", fmt.Sprintf("Recovered %d files", total), 100)
	return nil
}

func countFiles(dir string) int {
	count := 0
	filepath.Walk(dir, func(path string, info os.FileInfo, err error) error {
		if err == nil && !info.IsDir() {
			count++
		}
		return nil
	})
	return count
}
