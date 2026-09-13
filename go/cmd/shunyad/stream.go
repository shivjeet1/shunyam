package main

import (
	"context"
	"log"
	"time"

	pb "shunya/shunya/v1"
)

// StreamEvents streams live events from the machine to the UI
func (s *DaemonServer) StreamEvents(req *pb.StreamEventsRequest, stream pb.JobService_StreamEventsServer) error {
	jobID := req.JobId
	
	// Fast-forward or simulate events if job doesn't exist just for the UI
	// Real implementation would pull from s.machine event bus.
	// For MVP integration, we simulate the 3 steps: Pending, Wiping, Verifying
	
	log.Printf("Streaming events for job %s", jobID)
	
	steps := []string{"Pending", "Wiping", "Verifying", "Done"}

	for _, step := range steps {
		// Mock time taken per step
		time.Sleep(1 * time.Second)
		
		if step == "Wiping" {
			for i := 0; i <= 100; i += 20 {
				stream.Send(&pb.JobEvent{
					JobId:           jobID,
					StepName:        "Wiping",
					ProgressPercent: float32(i),
					Status:          "InProgress",
					Message:         "Overwriting sectors...",
				})
				time.Sleep(500 * time.Millisecond)
			}
		} else if step == "Verifying" {
			stream.Send(&pb.JobEvent{
				JobId:           jobID,
				StepName:        "Verifying",
				ProgressPercent: 50.0, // Partial progress for carving
				Status:          "InProgress",
				Message:         "Carving structures...",
			})
			time.Sleep(2 * time.Second)
			stream.Send(&pb.JobEvent{
				JobId:           jobID,
				StepName:        "Verifying",
				ProgressPercent: 100.0,
				Status:          "InProgress",
				Message:         "Carving complete",
			})
		} else if step == "Done" {
			stream.Send(&pb.JobEvent{
				JobId:           jobID,
				StepName:        "Done",
				ProgressPercent: 100.0,
				Status:          "Success",
				Message:         "Wipe Complete",
			})
		}
	}

	return nil
}

// GetCertificate triggers the certificate generation process
func (s *DaemonServer) GetCertificate(ctx context.Context, req *pb.GetCertificateRequest) (*pb.GetCertificateResponse, error) {
	log.Printf("Generating certificate for job %s", req.JobId)
	
	// Real implementation calls `shunya-engine generate-cert`
	// Here we simulate the RPC response
	time.Sleep(1 * time.Second)
	
	return &pb.GetCertificateResponse{
		CertJson:  "{\"status\": \"certified\"}",
		PdfData:   []byte("PDF_MOCK_DATA"),
		QrPayload: "shunya://cert/" + req.JobId,
	}, nil
}
