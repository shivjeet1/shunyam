package job

import (
	"fmt"
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

// SimulateExecution is a mock executor for the MVP that simulates the state transitions.
// In reality, this would spawn the Rust engine and read its events over grpc/stdout.
func (m *Machine) SimulateExecution(jobID string) {
	// Probing
	m.Transition(jobID, StateProbing, 10.0)

	// Wiping
	m.Transition(jobID, StateWiping, 20.0)
	// (Simulate wipe progress)
	m.Transition(jobID, StateWiping, 50.0)

	// Verifying
	m.Transition(jobID, StateVerifying, 80.0)

	// Carving
	m.Transition(jobID, StateCarving, 90.0)

	// Done
	m.Transition(jobID, StateDone, 100.0)
	fmt.Printf("[Job %s] Execution completed successfully.\n", jobID)
}
