package wipe

import (
	"encoding/json"
	"fmt"
	"os/exec"
	"regexp"
	"time"
)

func controllerFromNamespace(namespaceDev string) string {
	re := regexp.MustCompile(`(/dev/nvme\d+)n\d+`)
	matches := re.FindStringSubmatch(namespaceDev)
	if len(matches) > 1 {
		return matches[1]
	}
	return namespaceDev
}

func ExecuteSanitize(devicePath string, action string, pattern uint32, passes int, invertPattern bool, emit EmitFunc, jobID string) error {
	ctrl := controllerFromNamespace(devicePath)
	args := []string{"sanitize", ctrl, "-a", action}
	if action == "start-overwrite" {
		args = append(args, "-p", fmt.Sprintf("0x%x", pattern), "-n", fmt.Sprintf("%d", passes))
		if invertPattern {
			args = append(args, "-i")
		}
	}

	cmd := exec.Command("nvme", args...)
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("nvme sanitize failed: %w", err)
	}

	return pollSanitizeProgress(ctrl, emit, jobID)
}

func ExecuteFormat(devicePath string, ses int, emit EmitFunc, jobID string) error {
	args := []string{"format", devicePath, fmt.Sprintf("--ses=%d", ses)}
	cmd := exec.Command("nvme", args...)
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("nvme format failed: %w", err)
	}
	return nil
}

func pollSanitizeProgress(controllerDev string, emit EmitFunc, jobID string) error {
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

		emit(jobID, "Wiping", "InProgress", "Sanitize in progress...", progress)

		if status == 0x1 {
			break // completed
		}
		if status == 0x3 {
			return fmt.Errorf("nvme sanitize failed (sstat=3)")
		}

		time.Sleep(2 * time.Second)
	}
	return nil
}
