package db

import (
	"database/sql"
	"fmt"
	"os"
	"path/filepath"

	_ "github.com/mattn/go-sqlite3"
)

// Store encapsulates the database interaction
type Store struct {
	db *sql.DB
}

// InitStore creates the database file if it doesn't exist and runs migrations
func InitStore(dbPath string) (*Store, error) {
	if err := os.MkdirAll(filepath.Dir(dbPath), 0755); err != nil {
		return nil, fmt.Errorf("failed to create db directory: %w", err)
	}

	db, err := sql.Open("sqlite3", dbPath)
	if err != nil {
		return nil, err
	}

	// Schema creation
	schema := `
	CREATE TABLE IF NOT EXISTS jobs (
		id TEXT PRIMARY KEY,
		device_id TEXT NOT NULL,
		state TEXT NOT NULL,
		progress REAL DEFAULT 0.0,
		created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
		updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
		error_message TEXT
	);
	`
	if _, err := db.Exec(schema); err != nil {
		return nil, fmt.Errorf("failed to run migrations: %w", err)
	}

	return &Store{db: db}, nil
}

func (s *Store) InsertJob(jobID, deviceID, state string) error {
	query := `INSERT INTO jobs (id, device_id, state) VALUES (?, ?, ?)`
	_, err := s.db.Exec(query, jobID, deviceID, state)
	return err
}

func (s *Store) UpdateJobState(jobID, state string, progress float64, errMsg string) error {
	query := `UPDATE jobs SET state = ?, progress = ?, error_message = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?`
	_, err := s.db.Exec(query, state, progress, errMsg, jobID)
	return err
}

func (s *Store) GetJobState(jobID string) (string, float64, string, error) {
	var state string
	var progress float64
	var errMsg sql.NullString
	query := `SELECT state, progress, error_message FROM jobs WHERE id = ?`
	
	err := s.db.QueryRow(query, jobID).Scan(&state, &progress, &errMsg)
	if err != nil {
		return "", 0, "", err
	}
	return state, progress, errMsg.String, nil
}
