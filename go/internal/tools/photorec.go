package tools

import (
	"os/exec"
)

type PhotoRecAdapter struct {
	binaryPath string
}

func NewPhotoRecAdapter(binaryPath string) *PhotoRecAdapter {
	if binaryPath == "" {
		binaryPath = "photorec" // assuming testdisk package is installed
	}
	return &PhotoRecAdapter{binaryPath: binaryPath}
}

// RunCarving executes PhotoRec on a given device or image and places recovered files in outDir.
// It uses the non-interactive "/cmd" scripted run.
func (a *PhotoRecAdapter) RunCarving(source string, outDir string, profile string) error {
	// Base options
	cmdStr := "partition_none,options,paranoid,keep_corrupted_file,fileopt,everything,enable,wholespace,search"

	if profile == "triage" {
		cmdStr = "partition_none,options,freespace,search" // Simplified
	}

	cmd := exec.Command(
		a.binaryPath,
		"/log",
		"/d", outDir,
		"/cmd", source,
		cmdStr,
	)

	// PhotoRec writes its DFXML report to the output directory (report.xml).
	if err := cmd.Run(); err != nil {
		return err
	}

	return nil
}
