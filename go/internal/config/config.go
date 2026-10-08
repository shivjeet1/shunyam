package config

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

// Config holds all daemon configuration
type Config struct {
	// Daemon settings
	Address string `json:"address"` // gRPC listen address
	DBPath  string `json:"db_path"`  // SQLite database path

	// Engine settings
	EnginePath string `json:"engine_path"` // Path to shunya-engine binary

	// Wipe settings
	DefaultMethod string `json:"default_method"` // Default wipe method
	VerifySamples int    `json:"verify_samples"`  // Number of sectors to sample during verification

	// Security settings
	ChallengeTTLMinutes int  `json:"challenge_ttl_minutes"` // Challenge expiry time
	RequireLocalPresence bool `json:"require_local_presence"` // Require physical presence challenge

	// Logging settings
	LogLevel string `json:"log_level"` // debug, info, warn, error
}

// DefaultConfig returns a config with sensible defaults
func DefaultConfig() *Config {
	home, _ := os.UserHomeDir()
	return &Config{
		Address:              "127.0.0.1:9090",
		DBPath:               filepath.Join(home, ".local", "share", "shunya", "shunya.db"),
		EnginePath:           "",
		DefaultMethod:        "Single Pass",
		VerifySamples:        50,
		ChallengeTTLMinutes:  5,
		RequireLocalPresence: true,
		LogLevel:             "info",
	}
}

// Load reads config from the specified path. If the file doesn't exist,
// it returns DefaultConfig. Environment variables override file values.
func Load(path string) (*Config, error) {
	cfg := DefaultConfig()

	if path != "" {
		data, err := os.ReadFile(path)
		if err == nil {
			if err := json.Unmarshal(data, cfg); err != nil {
				return nil, fmt.Errorf("failed to parse config file: %w", err)
			}
		} else if !os.IsNotExist(err) {
			return nil, fmt.Errorf("failed to read config file: %w", err)
		}
	}

	// Environment variable overrides
	if v := os.Getenv("SHUNYA_ADDRESS"); v != "" {
		cfg.Address = v
	}
	if v := os.Getenv("SHUNYA_DB_PATH"); v != "" {
		cfg.DBPath = v
	}
	if v := os.Getenv("SHUNYA_ENGINE_PATH"); v != "" {
		cfg.EnginePath = v
	}
	if v := os.Getenv("SHUNYA_DEFAULT_METHOD"); v != "" {
		cfg.DefaultMethod = v
	}
	if v := os.Getenv("SHUNYA_VERIFY_SAMPLES"); v != "" {
		fmt.Sscanf(v, "%d", &cfg.VerifySamples)
	}
	if v := os.Getenv("SHUNYA_CHALLENGE_TTL"); v != "" {
		fmt.Sscanf(v, "%d", &cfg.ChallengeTTLMinutes)
	}
	if v := os.Getenv("SHUNYA_REQUIRE_PRESENCE"); v != "" {
		cfg.RequireLocalPresence = v != "false" && v != "0"
	}
	if v := os.Getenv("SHUNYA_LOG_LEVEL"); v != "" {
		cfg.LogLevel = v
	}

	return cfg, nil
}

// Save writes the config to the specified path
func (c *Config) Save(path string) error {
	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return err
	}

	data, err := json.MarshalIndent(c, "", "  ")
	if err != nil {
		return err
	}

	return os.WriteFile(path, data, 0644)
}
