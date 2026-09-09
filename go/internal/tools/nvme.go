package tools

import (
	"encoding/json"
	"os/exec"
)

type NVMEDevice struct {
	DevicePath string
	ModelNumber string `json:"ModelNumber"`
	SerialNumber string `json:"SerialNumber"`
	Firmware    string
}

type NVMEListOutput struct {
	Devices []struct {
		DevicePath   string `json:"DevicePath"`
		Firmware     string `json:"Firmware"`
		ModelNumber  string `json:"ModelNumber"`
		SerialNumber string `json:"SerialNumber"`
	} `json:"Devices"`
}

type NVMEAdapter struct {
	binaryPath string
}

func NewNVMEAdapter(binaryPath string) *NVMEAdapter {
	if binaryPath == "" {
		binaryPath = "nvme"
	}
	return &NVMEAdapter{binaryPath: binaryPath}
}

// ListDevices runs `nvme list -o json` and returns the devices.
func (a *NVMEAdapter) ListDevices() ([]NVMEDevice, error) {
	cmd := exec.Command(a.binaryPath, "list", "-o", "json")
	output, err := cmd.Output()
	if err != nil {
		return nil, err
	}

	var listOutput NVMEListOutput
	if err := json.Unmarshal(output, &listOutput); err != nil {
		return nil, err
	}

	var devices []NVMEDevice
	for _, dev := range listOutput.Devices {
		devices = append(devices, NVMEDevice{
			DevicePath:   dev.DevicePath,
			ModelNumber:  dev.ModelNumber,
			SerialNumber: dev.SerialNumber,
			Firmware:     dev.Firmware,
		})
	}
	return devices, nil
}
