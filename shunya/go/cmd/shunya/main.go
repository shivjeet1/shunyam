package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"net"
	"os"
	"time"

	"github.com/spf13/cobra"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"

	pb "shunya/shunya/v1"
)

var rootCmd = &cobra.Command{
	Use:   "shunya",
	Short: "Shunya Secure Wipe Platform CLI",
}

var listCmd = &cobra.Command{
	Use:   "list",
	Short: "List all storage devices",
	Run: func(cmd *cobra.Command, args []string) {
		conn, err := grpc.Dial(
			"passthrough:///unix:///tmp/shunyad.sock",
			grpc.WithTransportCredentials(insecure.NewCredentials()),
			grpc.WithContextDialer(func(ctx context.Context, addr string) (net.Conn, error) {
				var d net.Dialer
				return d.DialContext(ctx, "unix", "/tmp/shunyad.sock")
			}),
		)
		if err != nil {
			log.Fatalf("did not connect: %v", err)
		}
		defer conn.Close()

		c := pb.NewDeviceServiceClient(conn)
		ctx, cancel := context.WithTimeout(context.Background(), time.Second*5)
		defer cancel()

		r, err := c.ListDevices(ctx, &pb.ListDevicesRequest{})
		if err != nil {
			log.Fatalf("could not list devices: %v", err)
		}

		jsonOutput, _ := cmd.Flags().GetBool("json")
		if jsonOutput {
			out, _ := json.MarshalIndent(r.Devices, "", "  ")
			fmt.Println(string(out))
			return
		}

		fmt.Printf("%-15s %-10s %-20s %-20s\n", "ID", "CLASS", "MODEL", "SERIAL")
		fmt.Println("----------------------------------------------------------------------")
		for _, dev := range r.Devices {
			fmt.Printf("%-15s %-10s %-20s %-20s\n", dev.Id, dev.Class, dev.Model, dev.Serial)
		}
	},
}

func init() {
	listCmd.Flags().BoolP("json", "j", false, "Output in JSON format")
	rootCmd.AddCommand(listCmd)
}

func main() {
	if err := rootCmd.Execute(); err != nil {
		os.Exit(1)
	}
}
