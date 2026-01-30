-- Add daemon_id to database_servers table to know which daemon manages the database container
ALTER TABLE database_servers ADD COLUMN daemon_id UUID REFERENCES daemons(id) ON DELETE SET NULL;

-- Remove the unique constraint on db_type to allow multiple servers of the same type
ALTER TABLE database_servers DROP CONSTRAINT IF EXISTS database_servers_db_type_key;

-- Create index for daemon_id lookups
CREATE INDEX idx_database_servers_daemon_id ON database_servers(daemon_id);
