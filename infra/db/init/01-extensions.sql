-- Enable required Postgres extensions for Design Studio AI.
-- Runs once on first DB initialization (docker-entrypoint-initdb.d).
CREATE EXTENSION IF NOT EXISTS vector;   -- pgvector: semantic search over assets/artifacts
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
