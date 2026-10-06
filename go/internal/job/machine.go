package job

import (
	"log"
	"shunya/internal/db"
)

type State string

const (
	StatePending   State = "Pending"
	StateProbing   State = "Probing"
	StateWiping    State = "Wiping"
	StateVerifying State = "Verifying"
	StateCarving   State = "Carving"
	StateDone      State = "Done"
	StateFailed    State = "Failed"
)

// Machine tracks and executes state transitions for a wipe job
type Machine struct {
	store *db.Store
}

func NewMachine(store *db.Store) *Machine {
	return &Machine{store: store}
}

// CreateJob inserts a new job in the Pending state
func (m *Machine) CreateJob(jobID, deviceID string) error {
	log.Printf("[Job %s] Created for device %s", jobID, deviceID)
	return m.store.InsertJob(jobID, deviceID, string(StatePending))
}

// Transition moves the job to a new state and updates progress
func (m *Machine) Transition(jobID string, newState State, progress float64) error {
	log.Printf("[Job %s] Transitioning to %s (Progress: %.2f%%)", jobID, newState, progress)
	return m.store.UpdateJobState(jobID, string(newState), progress, "")
}

// Fail moves the job to Failed state with an error message
func (m *Machine) Fail(jobID string, reason error) error {
	log.Printf("[Job %s] FAILED: %v", jobID, reason)
	return m.store.UpdateJobState(jobID, string(StateFailed), 0.0, reason.Error())
}

// GetState returns the current state, progress, and error message of a job
func (m *Machine) GetState(jobID string) (string, float64, string, error) {
	return m.store.GetJobState(jobID)
}
