package tools

import (
	"encoding/json"
	"fmt"
	"os/exec"
	"strings"
)

// BlockDevice is a unified representation of any block device
// regardless of transport (NVMe, SATA, USB, etc.)
type BlockDevice struct {
	DevicePath   string
	Model        string
	Vendor       string
	Serial       string
	Transport    string // "nvme", "usb", "sata", "unknown"
	SizeBytes    uint64
	IsRotational bool
	IsBoot       bool
	IsSystem     bool
}

// lsblkDevice mirrors the JSON output of lsblk -b -J
type lsblkDevice struct {
	Name     string        `json:"name"`
	Type     string        `json:"type"`
	Size     uint64        `json:"size"`
	Model    string        `json:"model"`
	Vendor   string        `json:"vendor"`
	Serial   string        `json:"serial"`
	Tran     string        `json:"tran"`
	Rota     bool          `json:"rota"`
	Children []lsblkDevice `json:"children"`
}

type lsblkOutput struct {
	Blockdevices []lsblkDevice `json:"blockdevices"`
}

// nvmeListDevice mirrors the JSON output of nvme list -o json
type nvmeListOutput struct {
	Devices []struct {
		DevicePath   string `json:"DevicePath"`
		ModelNumber  string `json:"ModelNumber"`
		SerialNumber string `json:"SerialNumber"`
		Firmware     string `json:"Firmware"`
		PhysicalSize uint64 `json:"PhysicalSize"`
	} `json:"Devices"`
}

// ListAllBlockDevices uses lsblk (zero-privilege) to enumerate all block
// devices and overlays NVMe-specific data from nvme-cli where available.
func ListAllBlockDevices() ([]BlockDevice, error) {
	// 1. Primary enumeration via lsblk — works for all transports, no root needed.
	raw, err := exec.Command(
		"lsblk", "-b", "-J",
		"-o", "NAME,TYPE,SIZE,MODEL,VENDOR,SERIAL,TRAN,ROTA",
	).Output()
	if err != nil {
		return nil, fmt.Errorf("lsblk failed: %w", err)
	}

	var lsblk lsblkOutput
	if err := json.Unmarshal(raw, &lsblk); err != nil {
		return nil, fmt.Errorf("lsblk parse error: %w", err)
	}

	// 2. Best-effort NVMe overlay for firmware / exact capacity info.
	nvmeMap := buildNVMeMap()

	// 3. Read which devices are mounted (to flag system / boot disks).
	mountedDevs := getMountedDevices()

	var result []BlockDevice
	for _, d := range lsblk.Blockdevices {
		if d.Type != "disk" {
			continue
		}
		devPath := "/dev/" + d.Name

		dev := BlockDevice{
			DevicePath:   devPath,
			Model:        strings.TrimSpace(d.Model),
			Vendor:       strings.TrimSpace(d.Vendor),
			Serial:       strings.TrimSpace(d.Serial),
			Transport:    strings.TrimSpace(d.Tran),
			SizeBytes:    d.Size,
			IsRotational: d.Rota,
		}

		// Overlay NVMe data if available
		if nd, ok := nvmeMap[devPath]; ok {
			if dev.Model == "" {
				dev.Model = nd.ModelNumber
			}
			if dev.Serial == "" {
				dev.Serial = nd.SerialNumber
			}
			if nd.PhysicalSize > 0 {
				dev.SizeBytes = nd.PhysicalSize
			}
		}

		// Default model name if still empty
		if dev.Model == "" {
			dev.Model = d.Name
		}

		// Mark as system if any of its children are mounted at / or /boot
		for _, child := range d.Children {
			childPath := "/dev/" + child.Name
			if mp, ok := mountedDevs[childPath]; ok {
				if mp == "/" || mp == "/boot" || mp == "/boot/efi" {
					dev.IsSystem = true
					dev.IsBoot = (mp == "/boot" || mp == "/boot/efi")
				}
			}
		}
		// Also check if the disk itself is directly mounted
		if mp, ok := mountedDevs[devPath]; ok {
			if mp == "/" || mp == "/boot" || mp == "/boot/efi" {
				dev.IsSystem = true
			}
		}

		result = append(result, dev)
	}

	return result, nil
}

// buildNVMeMap runs nvme list -o json and returns a map keyed by DevicePath.
// Errors are silently ignored — nvme-cli may not be installed or may need root.
func buildNVMeMap() map[string]struct {
	ModelNumber  string
	SerialNumber string
	PhysicalSize uint64
} {
	m := make(map[string]struct {
		ModelNumber  string
		SerialNumber string
		PhysicalSize uint64
	})

	raw, err := exec.Command("nvme", "list", "-o", "json").Output()
	if err != nil {
		return m
	}

	var out nvmeListOutput
	if err := json.Unmarshal(raw, &out); err != nil {
		return m
	}

	for _, d := range out.Devices {
		m[d.DevicePath] = struct {
			ModelNumber  string
			SerialNumber string
			PhysicalSize uint64
		}{d.ModelNumber, d.SerialNumber, d.PhysicalSize}
	}
	return m
}

// getMountedDevices reads /proc/mounts and returns a map of device → mountpoint.
func getMountedDevices() map[string]string {
	result := make(map[string]string)
	raw, err := exec.Command("cat", "/proc/mounts").Output()
	if err != nil {
		return result
	}
	for _, line := range strings.Split(string(raw), "\n") {
		fields := strings.Fields(line)
		if len(fields) >= 2 {
			result[fields[0]] = fields[1]
		}
	}
	return result
}
