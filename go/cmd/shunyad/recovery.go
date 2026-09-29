package main

import (
	"context"

	"shunya/internal/recovery"
	pb "shunya/shunya/v1"
)

func (s *DaemonServer) StartRecovery(ctx context.Context, req *pb.StartRecoveryRequest) (*pb.StartRecoveryResponse, error) {
	jobID := "recovery-" + req.SourceDeviceId
	cfg := recovery.RecoveryConfig{
		DevicePath: req.SourceDeviceId,
		OutputDir:  req.OutputDirectory,
		Profile:    req.Profile,
		JobID:      jobID,
	}

	go func() {
		_ = recovery.Execute(cfg, func(jobID, status, message string, progress float32) {
			publish(jobID, &pb.JobEvent{
				JobId:           jobID,
				StepName:        "Recovery",
				Status:          status,
				Message:         message,
				ProgressPercent: progress,
			})
		})
	}()

	return &pb.StartRecoveryResponse{
		JobId: jobID,
	}, nil
}
