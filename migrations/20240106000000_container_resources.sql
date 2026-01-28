ALTER TABLE containers ADD COLUMN IF NOT EXISTS memory_limit BIGINT DEFAULT 512;
ALTER TABLE containers ADD COLUMN IF NOT EXISTS cpu_limit DECIMAL(4,2) DEFAULT 1.0;
ALTER TABLE containers ADD COLUMN IF NOT EXISTS disk_limit BIGINT DEFAULT 5120;
ALTER TABLE containers ADD COLUMN IF NOT EXISTS swap_limit BIGINT DEFAULT 0;
ALTER TABLE containers ADD COLUMN IF NOT EXISTS io_weight INTEGER DEFAULT 500;

CREATE TABLE IF NOT EXISTS container_ports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    container_id UUID NOT NULL REFERENCES containers(id) ON DELETE CASCADE,
    host_port INTEGER NOT NULL,
    container_port INTEGER NOT NULL,
    protocol VARCHAR(10) NOT NULL DEFAULT 'tcp',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(container_id, host_port, protocol)
);

CREATE INDEX IF NOT EXISTS idx_container_ports_container_id ON container_ports(container_id);
