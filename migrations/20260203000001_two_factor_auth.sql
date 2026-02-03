-- Add 2FA support to users table
ALTER TABLE users ADD COLUMN IF NOT EXISTS totp_secret TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS totp_enabled BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE users ADD COLUMN IF NOT EXISTS totp_verified_at TIMESTAMPTZ;
ALTER TABLE users ADD COLUMN IF NOT EXISTS backup_codes TEXT[];

-- Create 2FA recovery codes table for tracking used codes
CREATE TABLE IF NOT EXISTS totp_backup_codes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash VARCHAR(255) NOT NULL,
    used BOOLEAN NOT NULL DEFAULT FALSE,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_totp_backup_codes_user_id ON totp_backup_codes(user_id);

-- Create table for tracking 2FA login attempts (rate limiting)
CREATE TABLE IF NOT EXISTS totp_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ip_address VARCHAR(45),
    success BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_totp_attempts_user_id ON totp_attempts(user_id);
CREATE INDEX IF NOT EXISTS idx_totp_attempts_created_at ON totp_attempts(created_at);

-- Cleanup old attempts (can be run periodically)
-- DELETE FROM totp_attempts WHERE created_at < NOW() - INTERVAL '1 hour';
