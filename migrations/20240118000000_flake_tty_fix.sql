-- Fix tty column: ensure NOT NULL constraint and default value
-- This fixes the previous migration that was modified

-- Update any NULL values to false
UPDATE flakes SET tty = false WHERE tty IS NULL;

-- Alter column to be NOT NULL with default false (if not already)
ALTER TABLE flakes ALTER COLUMN tty SET NOT NULL;
ALTER TABLE flakes ALTER COLUMN tty SET DEFAULT false;

COMMENT ON COLUMN flakes.tty IS 'Whether to allocate a TTY for the container (needed for interactive programs like Hytale). Default is false for cleaner logs.';
