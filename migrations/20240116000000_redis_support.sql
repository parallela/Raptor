-- Add Redis support to database servers
ALTER TABLE database_servers DROP CONSTRAINT IF EXISTS database_servers_db_type_check;
ALTER TABLE database_servers ADD CONSTRAINT database_servers_db_type_check 
    CHECK (db_type IN ('postgresql', 'mysql', 'redis'));

-- Remove unique constraint on db_type to allow multiple servers of same type (optional)
-- ALTER TABLE database_servers DROP CONSTRAINT IF EXISTS database_servers_db_type_key;

-- Update user_databases constraint to include redis
ALTER TABLE user_databases DROP CONSTRAINT IF EXISTS user_databases_db_type_check;
ALTER TABLE user_databases ADD CONSTRAINT user_databases_db_type_check 
    CHECK (db_type IN ('postgresql', 'mysql', 'redis'));

-- For Redis, the database_name column will store the Redis DB number (auto-generated as string)
-- No additional columns needed - we'll use database_name to store the DB number for Redis
