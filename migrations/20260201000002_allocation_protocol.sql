-- Migration: Move protocol selection from flakes to allocations table
-- This allows users to select protocol (tcp/udp/both) per allocation instead of relying on flake defaults

-- Step 1: Add protocol column to allocations table if it doesn't exist
-- Values: 'tcp', 'udp', or 'both' (binds both TCP and UDP on the same port)
ALTER TABLE allocations ADD COLUMN IF NOT EXISTS protocol VARCHAR(10) NOT NULL DEFAULT 'tcp';

-- Step 2: Drop the default_protocol column from flakes (no longer needed)
ALTER TABLE flakes DROP COLUMN IF EXISTS default_protocol;

-- Step 3: Add comment explaining the protocol field
COMMENT ON COLUMN allocations.protocol IS 'Network protocol for this allocation: tcp, udp, or both (binds both TCP and UDP on the same port)';
