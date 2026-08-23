package main

import (
	"context"
	"log"
	"net"
	"os"
	"os/signal"
	"syscall"

	"google.golang.org/grpc"

	"shunya/internal/tools"
	pb "shunya/shunya/v1"
)

// DaemonServer implements the Shunya gRPC services
type DaemonServer struct {
	pb.UnimplementedDeviceServiceServer
	pb.UnimplementedJobServiceServer
	pb.UnimplementedRecoveryServiceServer
	pb.UnimplementedCertificateServiceServer
}

func (s *DaemonServer) ListDevices(ctx context.Context, req *pb.ListDevicesRequest) (*pb.ListDevicesResponse, error) {
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

func main() {
	log.Println("Starting shunyad...")

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
	srv := &DaemonServer{}

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
