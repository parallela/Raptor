CREATE TABLE IF NOT EXISTS ip_pools (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    daemon_id UUID NOT NULL REFERENCES daemons(id) ON DELETE CASCADE,
    ip_address VARCHAR(45) NOT NULL,
    cidr INTEGER DEFAULT 32,
    description VARCHAR(255),
    is_primary BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(daemon_id, ip_address)
);

CREATE INDEX IF NOT EXISTS idx_ip_pools_daemon_id ON ip_pools(daemon_id);

CREATE TABLE IF NOT EXISTS container_allocations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    container_id UUID NOT NULL REFERENCES containers(id) ON DELETE CASCADE,
    ip_pool_id UUID NOT NULL REFERENCES ip_pools(id) ON DELETE CASCADE,
    port INTEGER NOT NULL,
    internal_port INTEGER NOT NULL,
    protocol VARCHAR(10) NOT NULL DEFAULT 'tcp',
    is_primary BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(ip_pool_id, port, protocol)
);

CREATE INDEX IF NOT EXISTS idx_container_allocations_container_id ON container_allocations(container_id);
