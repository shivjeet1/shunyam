package main

import (
	"context"
	"log"
	"net"
	"os"
	"os/signal"
	"syscall"

	"github.com/google/uuid"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"

	"shunya/internal/db"
	"shunya/internal/job"
	"shunya/internal/policy"
	"shunya/internal/tools"
	pb "shunya/shunya/v1"
)

// DaemonServer implements the Shunya gRPC services
type DaemonServer struct {
	pb.UnimplementedDeviceServiceServer
	pb.UnimplementedJobServiceServer
	pb.UnimplementedRecoveryServiceServer
	pb.UnimplementedCertificateServiceServer
	gate    *policy.Gate
	machine *job.Machine
}

func (s *DaemonServer) ListDevices(ctx context.Context, req *pb.ListDevicesRequest) (*pb.ListDevicesResponse, error) {
	if tools.ShouldUseNativeProbe() {
		adapter := tools.NewRustEngineAdapter("shunya-engine")
		if err := adapter.ListDevices(); err != nil {
			log.Printf("Native Rust probing failed: %v", err)
		}
		return &pb.ListDevicesResponse{}, nil
	}

	adapter := tools.NewNVMEAdapter("nvme")
	nvmeDevs, err := adapter.ListDevices()
	if err != nil {
		log.Printf("Failed to list NVMe devices: %v", err)
		nvmeDevs = []tools.NVMEDevice{}
	}

	var devices []*pb.Device
	for _, dev := range nvmeDevs {
		devices = append(devices, &pb.Device{
			Id:               dev.DevicePath,
			Class:            "nvme-ssd",
			Model:            dev.ModelNumber,
			Serial:           dev.SerialNumber,
			Firmware:         dev.Firmware,
			SupportedMethods: []string{"nvme.sanitize.crypto", "nvme.sanitize.block"},
		})
	}

	return &pb.ListDevicesResponse{
		Devices: devices,
	}, nil
}

func (s *DaemonServer) StartJob(ctx context.Context, req *pb.StartJobRequest) (*pb.StartJobResponse, error) {
	isAuthorized := true
	if err := s.gate.CanWipe(req.DeviceId, isAuthorized); err != nil {
		log.Printf("Job rejected by policy gate for %s: %v", req.DeviceId, err)
		return nil, status.Errorf(codes.PermissionDenied, "policy violation: %v", err)
	}

	jobID := uuid.New().String()
	
	// Track job in SQLite via State Machine
	if err := s.machine.CreateJob(jobID, req.DeviceId); err != nil {
		return nil, status.Errorf(codes.Internal, "failed to create job: %v", err)
	}

	// Kick off the background execution
	if err := tools.StreamWipeJob(s.machine, req, jobID); err != nil {
			s.machine.Fail(jobID, err)
		}

	return &pb.StartJobResponse{
		JobId: jobID,
	}, nil
}

func main() {
	log.Println("Starting shunyad...")

	// Initialize DB
	dbPath := "/tmp/shunya.db"
	store, err := db.InitStore(dbPath)
	if err != nil {
		log.Fatalf("Failed to initialize database: %v", err)
	}

	socketPath := "/tmp/shunyad.sock"
	if err := os.RemoveAll(socketPath); err != nil {
		log.Fatalf("Failed to remove existing socket: %v", err)
	}

	lis, err := net.Listen("unix", socketPath)
	if err != nil {
		log.Fatalf("Failed to listen: %v", err)
	}

	if err := os.Chmod(socketPath, 0666); err != nil {
		log.Fatalf("Failed to chmod socket: %v", err)
	}

	grpcServer := grpc.NewServer()
	srv := &DaemonServer{
		gate:    policy.NewGate(true),
		machine: job.NewMachine(store),
	}

	pb.RegisterDeviceServiceServer(grpcServer, srv)
	pb.RegisterJobServiceServer(grpcServer, srv)
	pb.RegisterRecoveryServiceServer(grpcServer, srv)
	pb.RegisterCertificateServiceServer(grpcServer, srv)

	go func() {
		log.Printf("shunyad listening on %s", socketPath)
		if err := grpcServer.Serve(lis); err != nil {
			log.Fatalf("Failed to serve: %v", err)
		}
	}()

	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, os.Interrupt, syscall.SIGTERM)
	<-sigChan

	log.Println("Shutting down shunyad...")
	grpcServer.GracefulStop()
}
