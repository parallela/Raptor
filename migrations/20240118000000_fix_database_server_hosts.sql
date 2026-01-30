-- Update database_servers with localhost host to use their daemon's host
UPDATE database_servers ds
SET host = d.host
FROM daemons d
WHERE ds.daemon_id = d.id
AND ds.host = 'localhost';
