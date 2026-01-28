-- Add stop_command field to containers for graceful stop support
-- This command will be executed via stdin before stopping the container

ALTER TABLE containers ADD COLUMN IF NOT EXISTS stop_command VARCHAR(255) DEFAULT 'stop';
