package wipe

import (
	"bufio"
	"fmt"
	"os/exec"
	"regexp"
	"strconv"
	"strings"
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
		return fmt.Errorf("shred: failed to create stderr pipe: %w", err)
	}

	if err := cmd.Start(); err != nil {
		return fmt.Errorf("shred: failed to start: %w", err)
	}

	// shred -v uses \r for in-place progress updates, so we need a custom scanner
	scanner := bufio.NewScanner(stderr)
	scanner.Split(func(data []byte, atEOF bool) (advance int, token []byte, err error) {
		if atEOF && len(data) == 0 {
			return 0, nil, nil
		}
		// Split on \r or \n
		for i := 0; i < len(data); i++ {
			if data[i] == '\n' || data[i] == '\r' {
				return i + 1, data[:i], nil
			}
		}
		if atEOF {
			return len(data), data, nil
		}
		return 0, nil, nil
	})

	rePass := regexp.MustCompile(`pass (\d+)/(\d+)`)
	reSize := regexp.MustCompile(`\.\.\.(\d+(?:\.\d+)?)\s*(B|KiB|MiB|GiB|TiB|kB|MB|GB|TB)`)

	totalPassesCount := passes
	if zeroFinal {
		totalPassesCount++
	}

	// Get device size for progress calculation
	var deviceSizeBytes float64
	if sizeOut, err := exec.Command("blockdev", "--getsize64", devicePath).Output(); err == nil {
		fmt.Sscanf(strings.TrimSpace(string(sizeOut)), "%f", &deviceSizeBytes)
	}

	for scanner.Scan() {
		line := scanner.Text()
		if line == "" {
			continue
		}

		passMatches := rePass.FindStringSubmatch(line)
		if len(passMatches) < 3 {
			continue
		}

		currPass, _ := strconv.Atoi(passMatches[1])
		passMsg := fmt.Sprintf("Shred pass %d/%d", currPass, totalPassesCount)

		// Base progress: which pass are we in
		baseProgress := float32(currPass-1) * 100.0 / float32(totalPassesCount)

		sizeMatches := reSize.FindStringSubmatch(line)
		if len(sizeMatches) == 3 && deviceSizeBytes > 0 {
			sizeVal, _ := strconv.ParseFloat(sizeMatches[1], 64)
			unit := sizeMatches[2]
			var writtenBytes float64
			switch unit {
			case "B":
				writtenBytes = sizeVal
			case "kB":
				writtenBytes = sizeVal * 1000
			case "KiB":
				writtenBytes = sizeVal * 1024
			case "MB":
				writtenBytes = sizeVal * 1000000
			case "MiB":
				writtenBytes = sizeVal * 1024 * 1024
			case "GB":
				writtenBytes = sizeVal * 1000000000
			case "GiB":
				writtenBytes = sizeVal * 1024 * 1024 * 1024
			case "TB":
				writtenBytes = sizeVal * 1000000000000
			case "TiB":
				writtenBytes = sizeVal * 1024 * 1024 * 1024 * 1024
			}
			passFraction := float32(writtenBytes / deviceSizeBytes)
			if passFraction > 1.0 {
				passFraction = 1.0
			}
			baseProgress += passFraction * 100.0 / float32(totalPassesCount)
			passMsg = fmt.Sprintf("Shred pass %d/%d (%.1f%%)", currPass, totalPassesCount, passFraction*100)
		}

		emit(jobID, "Wiping", "InProgress", passMsg, baseProgress)
	}

	if err := cmd.Wait(); err != nil {
		return fmt.Errorf("shred failed: %w", err)
	}
	return nil
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
