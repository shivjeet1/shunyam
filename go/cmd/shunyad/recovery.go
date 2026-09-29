package main

import (
	"context"
	"fmt"
	"log"
	"os/exec"
	"shunya/internal/tools"
	"time"

	pb "shunya/shunya/v1"
)

func (s *DaemonServer) StartRecovery(ctx context.Context, req *pb.StartRecoveryRequest) (*pb.StartRecoveryResponse, error) {
	log.Printf("StartRecovery requested for device: %s, output: %s, profile: %s", req.SourceDeviceId, req.OutputDirectory, req.Profile)

	jobID := fmt.Sprintf("recovery-%s-%d", req.SourceDeviceId, time.Now().UnixMilli())

	go func() {
		log.Printf("Spawning shunya-engine for recovery job %s", jobID)
		cmd := exec.Command(tools.FindEngineBinary(), "recover", "--device", req.SourceDeviceId, "--output", req.OutputDirectory, "--profile", req.Profile)

		output, err := cmd.CombinedOutput()
		if err != nil {
			log.Printf("Warning: shunya-engine recover failed or not found for job %s: %v\nOutput: %s", jobID, err, string(output))
		} else {
			log.Printf("shunya-engine recover completed for job %s\nOutput: %s", jobID, string(output))
		}
	}()

	return &pb.StartRecoveryResponse{
		JobId: jobID,
	}, nil
}
