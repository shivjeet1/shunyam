package main

import (
	"bytes"
	"context"
	"fmt"
	"log"
	"os"
	"os/exec"
	"path/filepath"
	"sync"

	"shunya/internal/job"
	"shunya/internal/tools"
	"shunya/internal/wipe"
	pb "shunya/shunya/v1"
)

var eventBus = struct {
	sync.RWMutex
	subs map[string][]chan *pb.JobEvent
}{subs: make(map[string][]chan *pb.JobEvent)}

var jobMethods = struct {
	sync.RWMutex
	m map[string]string
}{m: make(map[string]string)}

var jobToDevice = struct {
	sync.RWMutex
	m map[string]string
}{m: make(map[string]string)}

func subscribe(jobID string) chan *pb.JobEvent {
	ch := make(chan *pb.JobEvent, 64)
	eventBus.Lock()
	eventBus.subs[jobID] = append(eventBus.subs[jobID], ch)
	eventBus.Unlock()
	return ch
}

func closeJobChannels(jobID string) {
	eventBus.Lock()
	defer eventBus.Unlock()
	for _, ch := range eventBus.subs[jobID] {
		close(ch)
	}
	delete(eventBus.subs, jobID)
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
	log.Printf("[Job %s] %s/%s: %s (%.1f%%)", jobID, step, status, msg, progress)
	publish(jobID, &pb.JobEvent{
		JobId:           jobID,
		StepName:        step,
		ProgressPercent: progress,
		Status:          status,
		Message:         msg,
	})
}

func runWipeJob(jobID string, method string, machine *job.Machine, req *pb.StartJobRequest) {
	defer closeJobChannels(jobID)

	if method == "" {
		method = "Single Pass"
	}

	jobMethods.Lock()
	jobMethods.m[jobID] = method
	jobMethods.Unlock()

	jobToDevice.Lock()
	jobToDevice.m[jobID] = req.DeviceId
	jobToDevice.Unlock()

	deviceRegistry.RLock()
	meta, ok := deviceRegistry.m[req.DeviceId]
	deviceRegistry.RUnlock()

	transport := "unknown"
	if ok {
		transport = meta.Transport
	}

	capacityBytes := req.CapacityBytes
	if ok && meta.Capacity > 0 {
		capacityBytes = meta.Capacity
	}

	cfg := wipe.WipeConfig{
		DevicePath:    req.DeviceId,
		Transport:     transport,
		Method:        method,
		CapacityBytes: capacityBytes,
		JobID:         jobID,
	}

	err := wipe.Execute(cfg, emit)
	if err != nil {
		log.Printf("[Job %s] WIPE FAILED: %v", jobID, err)
		emit(jobID, "Wiping", "Failed", err.Error(), 0)
		machine.Fail(jobID, err)
	} else {
		machine.Transition(jobID, job.StateDone, 100)
	}

	eventBus.Lock()
	for _, ch := range eventBus.subs[jobID] {
		close(ch)
	}
	delete(eventBus.subs, jobID)
	eventBus.Unlock()
}

func (s *DaemonServer) StreamEvents(req *pb.StreamEventsRequest, stream pb.JobService_StreamEventsServer) error {
	ch := subscribe(req.JobId)
	defer func() {
		// Clean up subscription if we exit early
		eventBus.Lock()
		subs := eventBus.subs[req.JobId]
		for i, sub := range subs {
			if sub == ch {
				eventBus.subs[req.JobId] = append(subs[:i], subs[i+1:]...)
				break
			}
		}
		eventBus.Unlock()
	}()

	// Always send current state immediately to avoid missing fast transitions
	state, prog, msg, err := s.machine.GetState(req.JobId)
	if err == nil {
		evt := &pb.JobEvent{
			JobId:           req.JobId,
			StepName:        state,
			ProgressPercent: float32(prog),
			Status:          "InProgress",
			Message:         msg,
		}
		if state == "Failed" {
			evt.Status = "Failed"
		} else if state == "Done" {
			evt.Status = "Success"
		}
		if err := stream.Send(evt); err != nil {
			return err
		}
		if state == "Done" || state == "Failed" {
			return nil
		}
	}

	for evt := range ch {
		if err := stream.Send(evt); err != nil {
			return err
		}
	}
	return nil
}

func (s *DaemonServer) GetCertificate(ctx context.Context, req *pb.GetCertificateRequest) (*pb.GetCertificateResponse, error) {
	jobMethods.RLock()
	method, ok := jobMethods.m[req.JobId]
	jobMethods.RUnlock()
	if !ok || method == "" {
		method = "Single Pass"
	}

	jobToDevice.RLock()
	deviceID := jobToDevice.m[req.JobId]
	jobToDevice.RUnlock()

	var devMeta DeviceMeta
	deviceRegistry.RLock()
	if meta, found := deviceRegistry.m[deviceID]; found {
		devMeta = meta
	}
	deviceRegistry.RUnlock()

	tempDir, err := os.MkdirTemp("", "shunya-cert-*")
	if err == nil {
		defer os.RemoveAll(tempDir)
		cmd := exec.Command(tools.FindEngineBinary(), "generate-cert",
			"--job-id", req.JobId,
			"--device-serial", devMeta.Serial,
			"--device-model", devMeta.Model,
			"--capacity", fmt.Sprintf("%d", devMeta.Capacity),
			"--operator", devMeta.Operator,
			"--out-dir", tempDir)

		if err := cmd.Run(); err == nil {
			jsonPath := filepath.Join(tempDir, req.JobId+".json")
			pdfPath := filepath.Join(tempDir, req.JobId+".pdf")

			jsonBytes, errJson := os.ReadFile(jsonPath)
			pdfBytes, errPdf := os.ReadFile(pdfPath)

			if errJson == nil && errPdf == nil {
				return &pb.GetCertificateResponse{
					CertJson:  string(jsonBytes),
					PdfData:   pdfBytes,
					QrPayload: "shunya://cert/" + req.JobId + "?method=" + method,
				}, nil
			}
		}
	}

	return nil, fmt.Errorf("failed to generate certificate")
}

func (s *DaemonServer) VerifyCertificate(ctx context.Context, req *pb.VerifyCertificateRequest) (*pb.VerifyCertificateResponse, error) {
	cmd := exec.Command(tools.FindEngineBinary(), "verify-cert", "--device", req.DeviceId)
	var out bytes.Buffer
	var stderr bytes.Buffer
	cmd.Stdout = &out
	cmd.Stderr = &stderr
	err := cmd.Run()
	return &pb.VerifyCertificateResponse{
		IsValid:      err == nil,
		ManifestJson: out.String(),
		ErrorMessage: stderr.String(),
	}, nil
}
