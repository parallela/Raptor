-- Add restart_policy column to flakes table
-- This allows each flake to specify its Docker restart policy:
-- - "unless-stopped": Restart unless explicitly stopped via Docker API (good for game servers like Minecraft)
-- - "on-failure": Restart only on non-zero exit code (good for Node.js, etc.)
-- - "no": Never restart automatically
-- - "always": Always restart

ALTER TABLE flakes ADD COLUMN IF NOT EXISTS restart_policy VARCHAR(50) DEFAULT 'unless-stopped';

-- Update existing flakes with appropriate restart policies
-- Game servers (Minecraft, Paper) should use "unless-stopped" so they restart when user types "stop"
UPDATE flakes SET restart_policy = 'unless-stopped' WHERE slug IN ('paper', 'paper-yolk', 'vanilla');

-- Node.js and other generic servers should use "on-failure" so they don't restart on manual stop
UPDATE flakes SET restart_policy = 'on-failure' WHERE slug IN ('nodejs-http');

-- Add comment for documentation
COMMENT ON COLUMN flakes.restart_policy IS 'Docker restart policy: no, always, on-failure, unless-stopped';
