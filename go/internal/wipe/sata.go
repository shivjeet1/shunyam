package wipe

import (
	"fmt"
	"os/exec"
	"strings"
)

func CheckATASecuritySupport(devicePath string) (supported, frozen bool, err error) {
	cmd := exec.Command("hdparm", "-I", devicePath)
	out, err := cmd.CombinedOutput()
	if err != nil {
		return false, false, err
	}
	output := string(out)
	
	if strings.Contains(output, "supported: enhanced erase") || strings.Contains(output, "supported: secure erase") {
		supported = true
	}
	if strings.Contains(output, "frozen") && !strings.Contains(output, "not frozen") {
		frozen = true
	}
	return supported, frozen, nil
}

func ExecuteSecureErase(devicePath string, enhanced bool, emit EmitFunc, jobID string) error {
	supported, frozen, err := CheckATASecuritySupport(devicePath)
	if err != nil {
		return fmt.Errorf("hdparm -I failed: %w", err)
	}
	if !supported {
		return fmt.Errorf("secure erase not supported on %s", devicePath)
	}
	if frozen {
		return fmt.Errorf("device is frozen, cannot secure erase")
	}

	emit(jobID, "Wiping", "InProgress", "Setting temporary security password...", 10.0)
	cmd := exec.Command("hdparm", "--user-master", "u", "--security-set-pass", "Eins", devicePath)
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("failed to set security password: %w", err)
	}

	emit(jobID, "Wiping", "InProgress", "Issuing secure erase command (this may take a long time)...", 20.0)
	eraseFlag := "--security-erase"
	if enhanced {
		eraseFlag = "--security-erase-enhanced"
	}
	cmd = exec.Command("hdparm", "--user-master", "u", eraseFlag, "Eins", devicePath)
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("secure erase failed: %w", err)
	}

	return nil
}
