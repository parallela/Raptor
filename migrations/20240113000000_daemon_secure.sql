-- Add secure field to daemons table for HTTPS support
ALTER TABLE daemons ADD COLUMN IF NOT EXISTS secure BOOLEAN NOT NULL DEFAULT false;

-- Add comment explaining the field
COMMENT ON COLUMN daemons.secure IS 'Whether to use HTTPS when connecting to this daemon';
