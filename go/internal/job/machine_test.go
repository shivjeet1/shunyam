package job

import (
	"fmt"
	"shunya/internal/db"
	"testing"
)

func setupTestDB(t *testing.T) *db.Store {
	t.Helper()
	store, err := db.InitStore(":memory:")
	if err != nil {
		t.Fatalf("failed to init test db: %v", err)
	}
	return store
}

func TestNewMachine(t *testing.T) {
	store := setupTestDB(t)
	m := NewMachine(store)
	if m == nil {
		t.Fatal("expected non-nil machine")
	}
	if m.store == nil {
		t.Error("expected store to be set")
	}
}

func TestCreateJob(t *testing.T) {
	store := setupTestDB(t)
	m := NewMachine(store)

	err := m.CreateJob("job-001", "/dev/sda")
	if err != nil {
		t.Fatalf("CreateJob failed: %v", err)
	}

	state, progress, errMsg, err := m.GetState("job-001")
	if err != nil {
		t.Fatalf("GetState failed: %v", err)
	}
	if state != string(StatePending) {
		t.Errorf("expected state Pending, got %s", state)
	}
	if progress != 0.0 {
		t.Errorf("expected progress 0.0, got %f", progress)
	}
	if errMsg != "" {
		t.Errorf("expected empty error message, got %s", errMsg)
	}
}

func TestTransition(t *testing.T) {
	store := setupTestDB(t)
	m := NewMachine(store)

	m.CreateJob("job-002", "/dev/sda")

	err := m.Transition("job-002", StateWiping, 50.0)
	if err != nil {
		t.Fatalf("Transition failed: %v", err)
	}

	state, progress, _, _ := m.GetState("job-002")
	if state != string(StateWiping) {
		t.Errorf("expected state Wiping, got %s", state)
	}
	if progress != 50.0 {
		t.Errorf("expected progress 50.0, got %f", progress)
	}
}

func TestTransitionSequence(t *testing.T) {
	store := setupTestDB(t)
	m := NewMachine(store)

	m.CreateJob("job-003", "/dev/sda")

	states := []State{StateProbing, StateWiping, StateVerifying, StateCarving, StateDone}
	for i, s := range states {
		progress := float64(i+1) * 20.0
		err := m.Transition("job-003", s, progress)
		if err != nil {
			t.Fatalf("Transition to %s failed: %v", s, err)
		}
	}

	state, progress, _, _ := m.GetState("job-003")
	if state != string(StateDone) {
		t.Errorf("expected final state Done, got %s", state)
	}
	if progress != 100.0 {
		t.Errorf("expected final progress 100.0, got %f", progress)
	}
}

func TestFail(t *testing.T) {
	store := setupTestDB(t)
	m := NewMachine(store)

	m.CreateJob("job-004", "/dev/sda")

	err := m.Fail("job-004", fmt.Errorf("test failure"))
	if err != nil {
		t.Fatalf("Fail failed: %v", err)
	}

	state, progress, errMsg, _ := m.GetState("job-004")
	if state != string(StateFailed) {
		t.Errorf("expected state Failed, got %s", state)
	}
	if progress != 0.0 {
		t.Errorf("expected progress 0.0, got %f", progress)
	}
	if errMsg != "test failure" {
		t.Errorf("expected error message 'test failure', got '%s'", errMsg)
	}
}

func TestGetStateNonExistent(t *testing.T) {
	store := setupTestDB(t)
	m := NewMachine(store)

	_, _, _, err := m.GetState("nonexistent-job")
	if err == nil {
		t.Error("expected error for non-existent job")
	}
}

func TestCreateJobDuplicate(t *testing.T) {
	store := setupTestDB(t)
	m := NewMachine(store)

	err := m.CreateJob("job-005", "/dev/sda")
	if err != nil {
		t.Fatalf("first CreateJob failed: %v", err)
	}

	// Creating a job with the same ID should fail (primary key constraint)
	err = m.CreateJob("job-005", "/dev/sda")
	if err == nil {
		t.Error("expected error for duplicate job ID")
	}
}

func TestStateConstants(t *testing.T) {
	expectedStates := map[State]string{
		StatePending:   "Pending",
		StateProbing:   "Probing",
		StateWiping:    "Wiping",
		StateVerifying: "Verifying",
		StateCarving:   "Carving",
		StateDone:      "Done",
		StateFailed:    "Failed",
	}

	for state, name := range expectedStates {
		if string(state) != name {
			t.Errorf("expected state %s, got %s", name, string(state))
		}
	}
}
