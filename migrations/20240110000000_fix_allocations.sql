-- Migration: Simplify allocations - store IP/port directly in container_allocations
-- Each container can have multiple allocations, with exactly one marked as primary
-- Removes ip_pool_id dependency and legacy container_id/allocation_id columns
-- This migration is idempotent - safe to run multiple times

-- Step 1: Add new columns to container_allocations if they don't exist
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'container_allocations' AND column_name = 'allocation_id') THEN
        ALTER TABLE container_allocations ADD COLUMN allocation_id UUID REFERENCES allocations(id) ON DELETE CASCADE;
    END IF;

    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'container_allocations' AND column_name = 'ip') THEN
        ALTER TABLE container_allocations ADD COLUMN ip VARCHAR(45);
    END IF;
END $$;

-- Step 2: Make ip_pool_id nullable if it exists and is NOT NULL
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'container_allocations' AND column_name = 'ip_pool_id' AND is_nullable = 'NO') THEN
        ALTER TABLE container_allocations ALTER COLUMN ip_pool_id DROP NOT NULL;
    END IF;
END $$;

-- Step 3: Migrate existing data - populate ip from ip_pools if exists and ip is null
UPDATE container_allocations ca
SET ip = ip.ip_address
FROM ip_pools ip
WHERE ca.ip_pool_id = ip.id AND ca.ip IS NULL;

-- Step 4: Check if allocations.container_id exists before migrating
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'allocations' AND column_name = 'container_id') THEN
        -- Migrate from allocations.container_id (legacy relationship)
        INSERT INTO container_allocations (id, container_id, allocation_id, ip, port, internal_port, protocol, is_primary, created_at)
        SELECT
            gen_random_uuid(),
            a.container_id,
            a.id,
            a.ip,
            a.port,
            a.port,
            'tcp',
            TRUE,
            NOW()
        FROM allocations a
        WHERE a.container_id IS NOT NULL
          AND NOT EXISTS (
            SELECT 1 FROM container_allocations ca
            WHERE ca.container_id = a.container_id AND ca.allocation_id = a.id
          );
    END IF;
END $$;

-- Step 5: Check if containers.allocation_id exists before migrating
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'containers' AND column_name = 'allocation_id') THEN
        -- Migrate from containers.allocation_id (alternative legacy relationship)
        INSERT INTO container_allocations (id, container_id, allocation_id, ip, port, internal_port, protocol, is_primary, created_at)
        SELECT
            gen_random_uuid(),
            c.id,
            c.allocation_id,
            a.ip,
            a.port,
            a.port,
            'tcp',
            TRUE,
            NOW()
        FROM containers c
        JOIN allocations a ON a.id = c.allocation_id
        WHERE c.allocation_id IS NOT NULL
          AND NOT EXISTS (
            SELECT 1 FROM container_allocations ca
            WHERE ca.container_id = c.id AND ca.allocation_id = c.allocation_id
          );
    END IF;
END $$;

-- Step 6: Remove legacy columns if they exist
ALTER TABLE allocations DROP COLUMN IF EXISTS container_id;
ALTER TABLE containers DROP COLUMN IF EXISTS allocation_id;

-- Step 7: Drop old constraints and indexes
DROP INDEX IF EXISTS idx_allocations_container_id;

-- Step 8: Drop ip_pool_id column (no longer needed)
ALTER TABLE container_allocations DROP COLUMN IF EXISTS ip_pool_id;

-- Step 9: Make ip NOT NULL if it isn't already (set default for any nulls first)
UPDATE container_allocations SET ip = '0.0.0.0' WHERE ip IS NULL;
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'container_allocations' AND column_name = 'ip' AND is_nullable = 'YES') THEN
        ALTER TABLE container_allocations ALTER COLUMN ip SET NOT NULL;
    END IF;
END $$;

-- Step 10: Create indexes for efficient lookups (if not exist)
CREATE INDEX IF NOT EXISTS idx_container_allocations_primary
    ON container_allocations(container_id, is_primary)
    WHERE is_primary = TRUE;

CREATE INDEX IF NOT EXISTS idx_container_allocations_allocation_id
    ON container_allocations(allocation_id);

-- Step 11: Ensure only one primary allocation per container (drop if exists first to avoid error)
DROP INDEX IF EXISTS idx_container_allocations_unique_primary;
CREATE UNIQUE INDEX idx_container_allocations_unique_primary
    ON container_allocations(container_id)
    WHERE is_primary = TRUE;

-- Step 12: Add unique constraint for ip/port/protocol per allocation
DROP INDEX IF EXISTS idx_container_allocations_unique_endpoint;
CREATE UNIQUE INDEX idx_container_allocations_unique_endpoint
    ON container_allocations(allocation_id, port, protocol)
    WHERE allocation_id IS NOT NULL;
