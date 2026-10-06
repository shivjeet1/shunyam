package main

import (
	"context"
	"encoding/json"
	"net"
	"os"
	"os/exec"
	"sync"

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

type DeviceMeta struct {
	DevicePath string
	Transport  string
	Model      string
	Serial     string
	Capacity   uint64
	Operator   string
}

var deviceRegistry = struct {
	sync.RWMutex
	m map[string]DeviceMeta
}{m: make(map[string]DeviceMeta)}

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
		return &pb.ListDevicesResponse{}, nil
	}

	var devices []*pb.Device
	for _, dev := range blockDevs {
		class := "block"
		switch dev.Transport {
		case "nvme":
			class = "nvme-ssd"
		case "usb":
			class = "usb-hdd"
		case "sata", "ata":
			class = "sata-hdd"
		}

		methods := []string{"Single Pass", "3-Pass", "Zero Fill"}
		if dev.Transport == "nvme" {
			// Query nvme capabilities
			cmd := exec.Command("nvme", "id-ctrl", dev.DevicePath, "-o", "json")
			if out, err := cmd.Output(); err == nil {
				var result struct {
					Sanicap uint32 `json:"sanicap"`
				}
				if json.Unmarshal(out, &result) == nil {
					if result.Sanicap&1 != 0 {
						methods = append(methods, "Crypto Erase")
					}
					if result.Sanicap&2 != 0 {
						methods = append(methods, "Block Erase")
					}
					if result.Sanicap&4 != 0 {
						methods = append(methods, "Overwrite")
					}
				}
			}
		} else if dev.Transport == "sata" || dev.Transport == "ata" {
            methods = append(methods, "Block Erase", "Crypto Erase")
        }

		deviceRegistry.Lock()
		deviceRegistry.m[dev.DevicePath] = DeviceMeta{
			DevicePath: dev.DevicePath,
			Transport:  dev.Transport,
			Model:      dev.Model,
			Serial:     dev.Serial,
			Capacity:   dev.SizeBytes,
		}
		deviceRegistry.Unlock()

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
	if err := s.gate.CanWipe(req.DeviceId, req.ChallengeResponse); err != nil {
		return nil, status.Errorf(codes.PermissionDenied, "policy violation: %v", err)
	}

	jobID := uuid.New().String()

	if err := s.machine.CreateJob(jobID, req.DeviceId); err != nil {
		return nil, status.Errorf(codes.Internal, "failed to create job: %v", err)
	}

	req.JobId = jobID
	go runWipeJob(jobID, req.RequestedMethod, s.machine, req)

	return &pb.StartJobResponse{JobId: jobID}, nil
}

func runServer(dbPath, socketPath string) (*grpc.Server, error) {
	store, err := db.InitStore(dbPath)
	if err != nil {
		return nil, err
	}

	var lis net.Listener
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
		grpcServer.Serve(lis)
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
