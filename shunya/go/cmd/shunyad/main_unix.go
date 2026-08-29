//go:build !windows

package main

import (
	"log"
	"os"
	"os/signal"
	"syscall"
)

func main() {
	log.Println("Starting shunyad (UNIX)...")

	dbPath := "/tmp/shunya.db"
	socketPath := "/tmp/shunyad.sock"

	server, err := runServer(dbPath, socketPath)
	if err != nil {
		log.Fatalf("Failed to run server: %v", err)
	}

	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, os.Interrupt, syscall.SIGTERM)
	<-sigChan

	log.Println("Shutting down shunyad...")
	server.GracefulStop()
}
