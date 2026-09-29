package main

import (
	"context"
	"log"
	"net"
	"os"

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
	blockDevs, err := tools.ListAllBlockDevices()
	if err != nil {
		log.Printf("ListAllBlockDevices failed: %v — returning empty list", err)
		return &pb.ListDevicesResponse{}, nil
	}

	var devices []*pb.Device
	for _, dev := range blockDevs {
		// Determine device class from transport
		class := "block"
		switch dev.Transport {
		case "nvme":
			class = "nvme-ssd"
		case "usb":
			class = "usb-hdd"
		case "sata", "ata":
			class = "sata-hdd"
		}

		// Supported wipe methods depend on transport
		methods := []string{"zero-fill", "chacha20-purge"}
		if dev.Transport == "nvme" {
			methods = append(methods, "nvme.sanitize.crypto", "nvme.sanitize.block")
		}

		devices = append(devices, &pb.Device{
			Id:               dev.DevicePath,
			Class:            class,
			Model:            dev.Model,
			Serial:           dev.Serial,
			CapacityBytes:    dev.SizeBytes,
			SupportedMethods: methods,
			IsSystemDisk:     dev.IsSystem,
			IsBootMedium:     dev.IsBoot,
		})
	}

	return &pb.ListDevicesResponse{Devices: devices}, nil
}

func (s *DaemonServer) StartJob(ctx context.Context, req *pb.StartJobRequest) (*pb.StartJobResponse, error) {
	challengeResponse := req.ChallengeResponse
	if err := s.gate.CanWipe(req.DeviceId, challengeResponse); err != nil {
		log.Printf("Job rejected by policy gate for %s: %v", req.DeviceId, err)
		return nil, status.Errorf(codes.PermissionDenied, "policy violation: %v", err)
	}

	jobID := uuid.New().String()

	if err := s.machine.CreateJob(jobID, req.DeviceId); err != nil {
		return nil, status.Errorf(codes.Internal, "failed to create job: %v", err)
	}
	
	// Fetch actual capacity
	if devs, err := tools.ListAllBlockDevices(); err == nil {
		for _, dev := range devs {
			if dev.DevicePath == req.DeviceId {
				req.CapacityBytes = dev.SizeBytes
				break
			}
		}
	}

	// Kick off the wipe in the background; StreamEvents will subscribe and relay events.
	method := req.RequestedMethod
	req.JobId = jobID
	go runWipeJob(jobID, method, s.machine, req)

	return &pb.StartJobResponse{
		JobId: jobID,
	}, nil
}

// runServer sets up and runs the gRPC server. It returns the running server so it can be gracefully stopped.
func runServer(dbPath, socketPath string) (*grpc.Server, error) {
	store, err := db.InitStore(dbPath)
	if err != nil {
		return nil, err
	}

	var lis net.Listener
	// Unix socket paths start with '/'. Everything else is treated as a TCP address.
	if len(socketPath) > 0 && socketPath[0] == '/' {
		os.RemoveAll(socketPath)
		lis, err = net.Listen("unix", socketPath)
		if err == nil {
			os.Chmod(socketPath, 0666)
		}
	} else {
		lis, err = net.Listen("tcp", socketPath)
	}

	if err != nil {
		return nil, err
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

	return grpcServer, nil
}

func (s *DaemonServer) GetChallenge(ctx context.Context, req *pb.GetChallengeRequest) (*pb.GetChallengeResponse, error) {
	challenge, err := s.gate.GenerateChallenge(req.DeviceId)
	if err != nil {
		return nil, status.Errorf(codes.Internal, "failed to generate challenge: %v", err)
	}
	return &pb.GetChallengeResponse{
		ChallengeString: challenge,
	}, nil
}
