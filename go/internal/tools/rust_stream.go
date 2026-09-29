package tools

import (
	"encoding/binary"
	"fmt"
	"io"
	"log"
	"os/exec"
	"shunya/internal/job"
	pb "shunya/shunya/v1"

	"google.golang.org/protobuf/proto"
)

// StreamWipeJob executes the Rust shunya-engine, sends the wipe request via stdin,
// and parses streamed JobEvent protobufs from stdout to update the Go state machine.
func StreamWipeJob(machine *job.Machine, req *pb.StartJobRequest, jobID string, onEvent func(*pb.JobEvent)) error {
	cmd := exec.Command(FindEngineBinary(), "stream")

	stdin, err := cmd.StdinPipe()
	if err != nil {
		return fmt.Errorf("failed to open stdin pipe: %w", err)
	}

	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return fmt.Errorf("failed to open stdout pipe: %w", err)
	}

	if err := cmd.Start(); err != nil {
		return fmt.Errorf("failed to start shunya-engine: %w", err)
	}

	// 1. Send the Request to Rust via stdin (Length-prefixed)
	reqBytes, err := proto.Marshal(req)
	if err != nil {
		return fmt.Errorf("failed to marshal request: %w", err)
	}

	// Write 4-byte big-endian length prefix
	var length uint32 = uint32(len(reqBytes))
	if err := binary.Write(stdin, binary.BigEndian, length); err != nil {
		return fmt.Errorf("failed to write length prefix: %w", err)
	}
	// Write payload
	if _, err := stdin.Write(reqBytes); err != nil {
		return fmt.Errorf("failed to write payload: %w", err)
	}

	// Close stdin so Rust knows we're done sending requests
	stdin.Close()

	// 2. Read streamed events from Rust via stdout (Length-prefixed)
	defer cmd.Wait()

	for {
			var length uint32
			if err := binary.Read(stdout, binary.BigEndian, &length); err != nil {
				if err == io.EOF {
					break // Stream closed cleanly
				}
				log.Printf("[Job %s] Error reading event length from engine: %v", jobID, err)
				machine.Fail(jobID, err)
				return err
			}

			payload := make([]byte, length)
			if _, err := io.ReadFull(stdout, payload); err != nil {
				log.Printf("[Job %s] Error reading event payload from engine: %v", jobID, err)
				machine.Fail(jobID, err)
				return err
			}

			var event pb.JobEvent
			if err := proto.Unmarshal(payload, &event); err != nil {
				log.Printf("[Job %s] Error unmarshaling event: %v", jobID, err)
				continue
			}

			// Map string status from JobEvent to State Machine
			var newState job.State
			switch event.Status {
			case "probing":
				newState = job.StateProbing
			case "wiping":
				newState = job.StateWiping
			case "verifying":
				newState = job.StateVerifying
			case "carving":
				newState = job.StateCarving
			case "success", "done":
				newState = job.StateDone
			case "failed", "error":
				newState = job.StateFailed
			default:
				log.Printf("[Job %s] Unknown status string: %s", jobID, event.Status)
				continue
			}

			if newState == job.StateFailed {
				machine.Fail(jobID, fmt.Errorf("engine failure: %s", event.Message))
			} else {
				machine.Transition(jobID, newState, float64(event.ProgressPercent))
			}
			
			event.JobId = jobID
			if onEvent != nil {
				onEvent(&event)
			}
		}
	return nil
}
