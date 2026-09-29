package wipe

import (
	"bufio"
	"fmt"
	"os/exec"
	"regexp"
	"strconv"
)

func ExecuteShred(devicePath string, passes int, zeroFinal bool, emit EmitFunc, jobID string) error {
	args := []string{"-v", "-n", fmt.Sprintf("%d", passes)}
	if zeroFinal {
		args = append(args, "-z")
	}
	args = append(args, devicePath)

	cmd := exec.Command("shred", args...)
	stderr, err := cmd.StderrPipe()
	if err != nil {
		return err
	}

	if err := cmd.Start(); err != nil {
		return err
	}

	scanner := bufio.NewScanner(stderr)
	re := regexp.MustCompile(`pass (\d+)/(\d+).* (\d+)%`)
	
	totalPassesCount := passes
	if zeroFinal {
		totalPassesCount++
	}

	for scanner.Scan() {
		line := scanner.Text()
		matches := re.FindStringSubmatch(line)
		if len(matches) == 4 {
			currPass, _ := strconv.Atoi(matches[1])
			passPercentFloat, _ := strconv.ParseFloat(matches[3], 32)
			
			progress := (float32(currPass-1) * 100.0 / float32(totalPassesCount)) + (float32(passPercentFloat) / float32(totalPassesCount))
			emit(jobID, "Wiping", "InProgress", fmt.Sprintf("Shred pass %d", currPass), progress)
		}
	}

	return cmd.Wait()
}

func ExecuteBlkdiscard(devicePath string, secure bool, emit EmitFunc, jobID string) error {
	args := []string{}
	if secure {
		args = append(args, "--secure")
	}
	args = append(args, devicePath)

	cmd := exec.Command("blkdiscard", args...)
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("blkdiscard failed: %w", err)
	}
	return nil
}

func ExecuteZeroFill(devicePath string, capacityBytes uint64, emit EmitFunc, jobID string) error {
	cmd := exec.Command("dd", "if=/dev/zero", "of="+devicePath, "bs=4M", "status=progress", "conv=fdatasync")
	stderr, err := cmd.StderrPipe()
	if err != nil {
		return err
	}

	if err := cmd.Start(); err != nil {
		return err
	}

	scanner := bufio.NewScanner(stderr)
	re := regexp.MustCompile(`(\d+) bytes`)

	for scanner.Scan() {
		line := scanner.Text()
		matches := re.FindStringSubmatch(line)
		if len(matches) == 2 {
			bytesCopied, _ := strconv.ParseUint(matches[1], 10, 64)
			var progress float32 = 0
			if capacityBytes > 0 {
				progress = float32(bytesCopied) * 100.0 / float32(capacityBytes)
			}
			emit(jobID, "Wiping", "InProgress", "Zero filling...", progress)
		}
	}

	return cmd.Wait()
}
