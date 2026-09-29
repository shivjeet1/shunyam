//go:build windows

package main

import (
	"log"
	"os"
	"path/filepath"

	"golang.org/x/sys/windows/svc"
	"google.golang.org/grpc"
)

type shunyaService struct{}

func (m *shunyaService) Execute(args []string, r <-chan svc.ChangeRequest, changes chan<- svc.Status) (ssec bool, errno uint32) {
	const cmdsAccepted = svc.AcceptStop | svc.AcceptShutdown
	changes <- svc.Status{State: svc.StartPending}

	// Use ProgramData for state instead of /tmp
	programData := os.Getenv("ProgramData")
	if programData == "" {
		programData = `C:\ProgramData`
	}
	dbPath := filepath.Join(programData, "Shunya", "shunya.db")

	// On Windows, use a localhost TCP socket instead of UNIX Domain Sockets
	socketPath := "127.0.0.1:9090"

	server, err := runServer(dbPath, socketPath)
	if err != nil {
		log.Printf("Failed to run server: %v", err)
		return
	}

	changes <- svc.Status{State: svc.Running, Accepts: cmdsAccepted}
	log.Println("Shunya service running on Windows...")

loop:
	for {
		select {
		case c := <-r:
			switch c.Cmd {
			case svc.Interrogate:
				changes <- c.CurrentStatus
			case svc.Stop, svc.Shutdown:
				log.Println("Shunya service stopping...")
				break loop
			default:
				log.Printf("Unexpected control request #%d", c)
			}
		}
	}

	changes <- svc.Status{State: svc.StopPending}
	server.GracefulStop()
	return
}

func main() {
	isInteractive, err := svc.IsAnInteractiveSession()
	if err != nil {
		log.Fatalf("Failed to determine if running in an interactive session: %v", err)
	}

	if isInteractive {
		log.Println("Running interactively, not as a Windows Service.")
		return
	}

	err = svc.Run("ShunyaWipeDaemon", &shunyaService{})
	if err != nil {
		log.Fatalf("Service failed: %v", err)
	}
}
