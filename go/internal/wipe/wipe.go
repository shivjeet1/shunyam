package wipe

import (
	"fmt"
	"strings"
)

type WipeConfig struct {
	DevicePath    string
	Transport     string
	Method        string
	CapacityBytes uint64
	JobID         string
}

type EmitFunc func(jobID, stepName, status, message string, progress float32)

func Execute(cfg WipeConfig, emit EmitFunc) error {
	emit(cfg.JobID, "Probing", "InProgress", "Probing device...", 5.0)

	var err error
	methodLower := strings.ToLower(cfg.Method)

	emit(cfg.JobID, "Wiping", "InProgress", "Starting wipe process...", 10.0)

	if strings.Contains(methodLower, "crypto erase") {
		if cfg.Transport == "nvme" {
			err = ExecuteSanitize(cfg.DevicePath, "start-crypto-erase", 0, 0, false, emit, cfg.JobID)
		} else if cfg.Transport == "sata" || cfg.Transport == "ata" {
			err = ExecuteSecureErase(cfg.DevicePath, true, emit, cfg.JobID)
		} else {
			err = fmt.Errorf("crypto erase not supported on %s", cfg.Transport)
		}
	} else if strings.Contains(methodLower, "block erase") || strings.Contains(methodLower, "purge") {
		if cfg.Transport == "nvme" {
			err = ExecuteSanitize(cfg.DevicePath, "start-block-erase", 0, 0, false, emit, cfg.JobID)
		} else if cfg.Transport == "sata" || cfg.Transport == "ata" {
			err = ExecuteSecureErase(cfg.DevicePath, false, emit, cfg.JobID)
		} else {
			err = ExecuteBlkdiscard(cfg.DevicePath, true, emit, cfg.JobID)
		}
	} else if strings.Contains(methodLower, "zero fill") || strings.Contains(methodLower, "clear") {
		if cfg.Transport == "nvme" {
			err = ExecuteFormat(cfg.DevicePath, 1, emit, cfg.JobID)
		} else {
			err = ExecuteZeroFill(cfg.DevicePath, cfg.CapacityBytes, emit, cfg.JobID)
		}
	} else if strings.Contains(methodLower, "3-pass") || strings.Contains(methodLower, "dod") {
		if cfg.Transport == "nvme" {
			err = ExecuteSanitize(cfg.DevicePath, "start-overwrite", 0x00, 3, true, emit, cfg.JobID)
		} else {
			err = ExecuteShred(cfg.DevicePath, 2, true, emit, cfg.JobID)
		}
	} else {
		// Single Pass / default
		if cfg.Transport == "nvme" {
			err = ExecuteSanitize(cfg.DevicePath, "start-overwrite", 0x5a5a5a5a, 1, false, emit, cfg.JobID)
		} else {
			err = ExecuteShred(cfg.DevicePath, 1, false, emit, cfg.JobID)
		}
	}

	if err != nil {
		emit(cfg.JobID, "Wiping", "Failed", err.Error(), 0.0)
		return err
	}

	emit(cfg.JobID, "Verifying", "InProgress", "Verifying wipe...", 90.0)
	if err := VerifyWipe(cfg.DevicePath, cfg.CapacityBytes, 50, emit, cfg.JobID); err != nil {
		emit(cfg.JobID, "Verifying", "Failed", err.Error(), 0.0)
		return err
	}

	emit(cfg.JobID, "Done", "Success", "Wipe completed successfully", 100.0)
	return nil
}
