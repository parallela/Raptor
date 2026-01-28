-- Add container_users table for many-to-many relationship between containers and users
CREATE TABLE IF NOT EXISTS container_users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    container_id UUID NOT NULL REFERENCES containers(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    permission_level VARCHAR(50) NOT NULL DEFAULT 'user',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(container_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_container_users_container_id ON container_users(container_id);
CREATE INDEX IF NOT EXISTS idx_container_users_user_id ON container_users(user_id);

-- Migrate existing user_id from containers to container_users (owner level)
INSERT INTO container_users (container_id, user_id, permission_level)
SELECT id, user_id, 'owner' FROM containers
ON CONFLICT (container_id, user_id) DO NOTHING;
