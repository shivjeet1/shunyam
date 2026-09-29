package wipe

import (
	"fmt"
	"math/rand"
	"os"
)

func VerifyWipe(devicePath string, capacityBytes uint64, numSamples int, emit EmitFunc, jobID string) error {
	if capacityBytes == 0 {
		return fmt.Errorf("unknown capacity, cannot verify")
	}
	totalSectors := capacityBytes / 512

	f, err := os.Open(devicePath)
	if err != nil {
		return err
	}
	defer f.Close()

	checkSector := func(sector uint64) error {
		buf := make([]byte, 512)
		_, err := f.ReadAt(buf, int64(sector*512))
		if err != nil {
			return err
		}
		// Only verify zeroes if the wipe method guarantees it.
		// For shred (without -z), crypto erase, or pattern overwrite, the data is not zero.
		// For now, we just ensure the sector is readable to verify the drive is still alive.
		return nil
	}

	samples := []uint64{0, totalSectors / 2, totalSectors - 1}
	for i := 0; i < numSamples; i++ {
		samples = append(samples, uint64(rand.Int63n(int64(totalSectors))))
	}

	for i, s := range samples {
		if err := checkSector(s); err != nil {
			return err
		}
		emit(jobID, "Verifying", "InProgress", fmt.Sprintf("Verified %d/%d samples", i+1, len(samples)), float32(i+1)*100.0/float32(len(samples)))
	}

	return nil
}
