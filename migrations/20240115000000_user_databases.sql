-- Shared Database Containers (managed by API, not per-user)
CREATE TABLE database_servers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    db_type VARCHAR(20) NOT NULL CHECK (db_type IN ('postgresql', 'mysql')),
    container_id VARCHAR(100),
    container_name VARCHAR(100) NOT NULL,
    host VARCHAR(255) NOT NULL DEFAULT 'localhost',
    port INTEGER NOT NULL,
    root_password VARCHAR(255) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'stopped',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(db_type)
);

-- User Database Instances (databases within shared containers)
CREATE TABLE user_databases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    server_id UUID NOT NULL REFERENCES database_servers(id) ON DELETE CASCADE,
    db_type VARCHAR(20) NOT NULL CHECK (db_type IN ('postgresql', 'mysql')),
    db_name VARCHAR(100) NOT NULL,
    db_user VARCHAR(100) NOT NULL,
    db_password VARCHAR(255) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_user_databases_user_id ON user_databases(user_id);
CREATE INDEX idx_user_databases_server_id ON user_databases(server_id);
CREATE INDEX idx_user_databases_status ON user_databases(status);
CREATE UNIQUE INDEX idx_user_databases_db_name ON user_databases(db_name);
CREATE UNIQUE INDEX idx_user_databases_db_user ON user_databases(db_user);

