package main

import (
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
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

	log.Printf("Spawning shunya-engine for recovery job %s", jobID)
	cmd := exec.Command(tools.FindEngineBinary(), "recover", "--device", req.SourceDeviceId, "--output", req.OutputDirectory, "--profile", req.Profile)

	output, err := cmd.CombinedOutput()
	if err != nil {
		log.Printf("shunya-engine recover failed: %v\nOutput: %s", err, string(output))
		return nil, status.Errorf(codes.Internal, "Recovery failed. Please ensure 'photorec' is installed on the system.")
	}

	log.Printf("shunya-engine recover completed for job %s\nOutput: %s", jobID, string(output))

	return &pb.StartRecoveryResponse{
		JobId: jobID,
	}, nil
}
