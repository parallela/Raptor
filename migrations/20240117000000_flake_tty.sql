-- Add tty column to flakes table for interactive container support
-- Some games like Hytale require a TTY for authentication
-- Default is false for cleaner logs

ALTER TABLE flakes ADD COLUMN IF NOT EXISTS tty BOOLEAN NOT NULL DEFAULT false;

-- Set all existing flakes to tty=false (they don't need TTY)
UPDATE flakes SET tty = false WHERE tty IS NULL;

COMMENT ON COLUMN flakes.tty IS 'Whether to allocate a TTY for the container (needed for interactive programs like Hytale). Default is false for cleaner logs.';
