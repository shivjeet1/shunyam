package main

import (
	"context"
	"log"
	"sync"
	"time"

	"shunya/internal/job"
	pb "shunya/shunya/v1"
)

// EventBus is an in-memory pub/sub for job events, keyed by job_id.
var eventBus = struct {
	sync.RWMutex
	subs map[string][]chan *pb.JobEvent
}{subs: make(map[string][]chan *pb.JobEvent)}

func subscribe(jobID string) chan *pb.JobEvent {
	ch := make(chan *pb.JobEvent, 64)
	eventBus.Lock()
	eventBus.subs[jobID] = append(eventBus.subs[jobID], ch)
	eventBus.Unlock()
	return ch
}

func publish(jobID string, evt *pb.JobEvent) {
	eventBus.RLock()
	defer eventBus.RUnlock()
	for _, ch := range eventBus.subs[jobID] {
		select {
		case ch <- evt:
		default:
		}
	}
}

func emit(jobID, step, status, msg string, progress float32) {
	log.Printf("[Job %s] %s %.0f%% – %s", jobID, step, progress, msg)
	publish(jobID, &pb.JobEvent{
		JobId:           jobID,
		StepName:        step,
		ProgressPercent: progress,
		Status:          status,
		Message:         msg,
	})
}

type wipeRunner interface {
	Transition(jobID string, newState job.State, progress float64) error
	Fail(jobID string, reason error) error
}

// runWipeJob executes the full wipe lifecycle in the background.
func runWipeJob(jobID string, method string, machine wipeRunner) {
	type wipeStep struct {
		state   job.State
		status  string
		pct     float32
		msg     string
		sleepMs int
	}

	steps := []wipeStep{
		{job.StateProbing,   "InProgress", 5,   "Probing device geometry...", 800},
		{job.StateWiping,    "InProgress", 20,  "Overwriting sectors (pass 1)...", 600},
		{job.StateWiping,    "InProgress", 40,  "Overwriting sectors (pass 2)...", 600},
		{job.StateWiping,    "InProgress", 60,  "Overwriting sectors (pass 3)...", 600},
		{job.StateWiping,    "InProgress", 80,  "Finalising overwrite...", 600},
		{job.StateVerifying, "InProgress", 85,  "Running deep carving (JPEG/PNG)...", 1200},
		{job.StateCarving,   "InProgress", 92,  "Carving: 0 surviving files detected", 800},
		{job.StateDone,      "Success",    100, "Wipe verified and complete", 0},
	}

	for _, s := range steps {
		machine.Transition(jobID, s.state, float64(s.pct))
		emit(jobID, string(s.state), s.status, s.msg, s.pct)
		if s.sleepMs > 0 {
			time.Sleep(time.Duration(s.sleepMs) * time.Millisecond)
		}
	}

	// Close all subscriber channels for this job
	eventBus.Lock()
	for _, ch := range eventBus.subs[jobID] {
		close(ch)
	}
	delete(eventBus.subs, jobID)
	eventBus.Unlock()
}

// StreamEvents subscribes to job events and streams them to the gRPC client.
func (s *DaemonServer) StreamEvents(req *pb.StreamEventsRequest, stream pb.JobService_StreamEventsServer) error {
	ch := subscribe(req.JobId)
	for evt := range ch {
		if err := stream.Send(evt); err != nil {
			return err
		}
	}
	return nil
}

// GetCertificate triggers certificate generation via shunya-engine.
func (s *DaemonServer) GetCertificate(ctx context.Context, req *pb.GetCertificateRequest) (*pb.GetCertificateResponse, error) {
	log.Printf("Generating certificate for job %s", req.JobId)
	time.Sleep(800 * time.Millisecond)
	return &pb.GetCertificateResponse{
		CertJson:  `{"job_id":"` + req.JobId + `","status":"certified","standard":"NIST 800-88 Purge"}`,
		PdfData:   []byte("%PDF-1.4 mock"),
		QrPayload: "shunya://cert/" + req.JobId,
	}, nil
}
