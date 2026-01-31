-- Add tty column to flakes table for interactive container support
-- Some games like Hytale require a TTY for authentication
-- Default is false for cleaner logs

-- Add the column if it doesn't exist
ALTER TABLE flakes ADD COLUMN IF NOT EXISTS tty BOOLEAN DEFAULT false;

-- Update any NULL values to false
UPDATE flakes SET tty = false WHERE tty IS NULL;

-- Set NOT NULL constraint
ALTER TABLE flakes ALTER COLUMN tty SET NOT NULL;
ALTER TABLE flakes ALTER COLUMN tty SET DEFAULT false;

COMMENT ON COLUMN flakes.tty IS 'Whether to allocate a TTY for the container (needed for interactive programs like Hytale). Default is false for cleaner logs.';
