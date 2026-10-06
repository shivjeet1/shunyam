package main

import (
	"context"
	"fmt"

	"github.com/google/uuid"
	"shunya/internal/recovery"
	pb "shunya/shunya/v1"
)

func (s *DaemonServer) StartRecovery(ctx context.Context, req *pb.StartRecoveryRequest) (*pb.StartRecoveryResponse, error) {
	jobID := "recovery-" + uuid.New().String()

	s.machine.CreateJob(jobID, req.SourceDeviceId)
	s.machine.Transition(jobID, "Recovery", 0)

	cfg := recovery.RecoveryConfig{
		DevicePath: req.SourceDeviceId,
		OutputDir:  req.OutputDirectory,
		Profile:    req.Profile,
		JobID:      jobID,
	}

	go func() {
		defer closeJobChannels(jobID)
		err := recovery.Execute(cfg, func(jID, status, message string, progress float32) {
			publish(jID, &pb.JobEvent{
				JobId:           jID,
				StepName:        "Recovery",
				Status:          status,
				Message:         message,
				ProgressPercent: progress,
			})
			if status == "Success" {
				s.machine.Transition(jID, "Done", 100)
			} else if status == "Failed" {
				
				s.machine.Fail(jID, fmt.Errorf("%s", message))
			} else {
				s.machine.Transition(jID, "Recovery", float64(progress))
			}
		})

		if err != nil {
			publish(jobID, &pb.JobEvent{
				JobId:           jobID,
				StepName:        "Recovery",
				Status:          "Failed",
				Message:         err.Error(),
				ProgressPercent: 0,
			})
			s.machine.Fail(jobID, err)
		}
	}()

	return &pb.StartRecoveryResponse{
		JobId: jobID,
	}, nil
}
