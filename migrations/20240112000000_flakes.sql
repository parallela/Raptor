-- Flakes (Server Templates) - Similar to Pterodactyl Eggs
-- Stores server type definitions with their docker images, startup commands, and variables

-- Flakes table (the main template definition)
CREATE TABLE IF NOT EXISTS flakes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(255) NOT NULL UNIQUE,
    author VARCHAR(255),
    description TEXT,

    -- Docker image to use
    docker_image VARCHAR(512) NOT NULL,

    -- Startup command with {{VARIABLE}} placeholders
    startup_command TEXT NOT NULL,

    -- Config for server files (JSON) - e.g., server.properties modifications
    config_files JSONB DEFAULT '{}',

    -- Startup detection - regex or string to detect when server is ready
    startup_detection VARCHAR(512),

    -- Installation script (optional) - runs when creating a new server
    install_script TEXT,
    install_container VARCHAR(512) DEFAULT 'artifacts.lstan.eu/java:21',
    install_entrypoint VARCHAR(255) DEFAULT '/bin/bash',

    -- Features flags (JSON array)
    features JSONB DEFAULT '[]',

    -- File denylist (JSON array of patterns)
    file_denylist JSONB DEFAULT '[]',

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Flake variables table
CREATE TABLE IF NOT EXISTS flake_variables (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    flake_id UUID NOT NULL REFERENCES flakes(id) ON DELETE CASCADE,

    name VARCHAR(255) NOT NULL,
    description TEXT,
    env_variable VARCHAR(255) NOT NULL,
    default_value TEXT DEFAULT '',

    -- Validation rules (simplified from Pterodactyl's Laravel rules)
    -- Options: required, nullable, string, numeric, boolean, min:X, max:X, regex:PATTERN
    rules VARCHAR(512) DEFAULT 'nullable|string',

    -- User permissions
    user_viewable BOOLEAN DEFAULT TRUE,
    user_editable BOOLEAN DEFAULT TRUE,

    -- Sort order
    sort_order INTEGER DEFAULT 0,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(flake_id, env_variable)
);

-- Container variables table (instance-specific variable values)
CREATE TABLE IF NOT EXISTS container_variables (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    container_id UUID NOT NULL REFERENCES containers(id) ON DELETE CASCADE,
    flake_variable_id UUID NOT NULL REFERENCES flake_variables(id) ON DELETE CASCADE,

    -- The actual value for this container
    value TEXT NOT NULL DEFAULT '',

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(container_id, flake_variable_id)
);

-- Add flake_id to containers table
ALTER TABLE containers ADD COLUMN IF NOT EXISTS flake_id UUID REFERENCES flakes(id) ON DELETE SET NULL;

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_flake_variables_flake_id ON flake_variables(flake_id);
CREATE INDEX IF NOT EXISTS idx_container_variables_container_id ON container_variables(container_id);
CREATE INDEX IF NOT EXISTS idx_containers_flake_id ON containers(flake_id);

-- Insert default Paper Minecraft flake
INSERT INTO flakes (id, name, slug, author, description, docker_image, startup_command, config_files, startup_detection, install_script, features)
VALUES (
    'f47ac10b-58cc-4372-a567-0e02b2c3d479',
    'Paper',
    'paper',
    'Raptor Panel',
    'High performance Spigot fork that aims to fix gameplay and mechanics inconsistencies.',
    'artifacts.lstan.eu/java:21',
    'java -Xms128M -Xmx{{SERVER_MEMORY}}M -Dterminal.jline=false -Dterminal.ansi=true -jar {{SERVER_JARFILE}}',
    '{
        "server.properties": {
            "parser": "properties",
            "find": {
                "server-ip": "0.0.0.0",
                "server-port": "{{server.build.default.port}}",
                "query.port": "{{server.build.default.port}}"
            }
        }
    }',
    ')! For help, type ',
    '#!/bin/bash
# Paper Installation Script
PROJECT=paper
cd /home/container

if [ -n "${DL_PATH}" ]; then
    echo "Using supplied download url: ${DL_PATH}"
    DOWNLOAD_URL=$(eval echo $(echo ${DL_PATH} | sed -e ''s/{{/${/g'' -e ''s/}}/}/g''))
else
    VER_EXISTS=$(curl -s https://api.papermc.io/v2/projects/${PROJECT} | jq -r --arg VERSION $MINECRAFT_VERSION ''.versions[] | contains($VERSION)'' | grep -m1 true)
    LATEST_VERSION=$(curl -s https://api.papermc.io/v2/projects/${PROJECT} | jq -r ''.versions'' | jq -r ''.[-1]'')

    if [ "${VER_EXISTS}" == "true" ]; then
        echo "Version is valid. Using version ${MINECRAFT_VERSION}"
    else
        echo "Specified version not found. Defaulting to the latest ${PROJECT} version"
        MINECRAFT_VERSION=${LATEST_VERSION}
    fi

    BUILD_EXISTS=$(curl -s https://api.papermc.io/v2/projects/${PROJECT}/versions/${MINECRAFT_VERSION} | jq -r --arg BUILD ${BUILD_NUMBER} ''.builds[] | tostring | contains($BUILD)'' | grep -m1 true)
    LATEST_BUILD=$(curl -s https://api.papermc.io/v2/projects/${PROJECT}/versions/${MINECRAFT_VERSION} | jq -r ''.builds'' | jq -r ''.[-1]'')

    if [ "${BUILD_EXISTS}" == "true" ]; then
        echo "Build is valid for version ${MINECRAFT_VERSION}. Using build ${BUILD_NUMBER}"
    else
        echo "Using the latest ${PROJECT} build for version ${MINECRAFT_VERSION}"
        BUILD_NUMBER=${LATEST_BUILD}
    fi

    JAR_NAME=${PROJECT}-${MINECRAFT_VERSION}-${BUILD_NUMBER}.jar

    echo "Version being downloaded"
    echo "MC Version: ${MINECRAFT_VERSION}"
    echo "Build: ${BUILD_NUMBER}"
    echo "JAR Name of Build: ${JAR_NAME}"
    DOWNLOAD_URL=https://api.papermc.io/v2/projects/${PROJECT}/versions/${MINECRAFT_VERSION}/builds/${BUILD_NUMBER}/downloads/${JAR_NAME}
fi

echo "Running curl -o ${SERVER_JARFILE} ${DOWNLOAD_URL}"

if [ -f ${SERVER_JARFILE} ]; then
    mv ${SERVER_JARFILE} ${SERVER_JARFILE}.old
fi

curl -o ${SERVER_JARFILE} ${DOWNLOAD_URL}

# Create eula.txt with eula=true
echo "eula=true" > eula.txt
echo "EULA accepted automatically"

if [ ! -f server.properties ]; then
    echo "Creating default server.properties"
    cat > server.properties << EOF
server-port=25565
query.port=25565
server-ip=0.0.0.0
EOF
fi',
    '["eula", "java_version"]'
) ON CONFLICT (slug) DO NOTHING;

-- Insert Paper flake variables
INSERT INTO flake_variables (flake_id, name, description, env_variable, default_value, rules, user_viewable, user_editable, sort_order)
VALUES
    ('f47ac10b-58cc-4372-a567-0e02b2c3d479', 'Server Memory', 'The maximum amount of memory (MB) the server can use.', 'SERVER_MEMORY', '1024', 'required|numeric|min:128', true, true, 0),
    ('f47ac10b-58cc-4372-a567-0e02b2c3d479', 'Minecraft Version', 'The version of Minecraft to download. Leave at latest to always get the latest version.', 'MINECRAFT_VERSION', 'latest', 'required|string|max:20', true, true, 1),
    ('f47ac10b-58cc-4372-a567-0e02b2c3d479', 'Server Jar File', 'The name of the server jarfile to run the server with.', 'SERVER_JARFILE', 'server.jar', 'required|string|max:100', true, true, 2),
    ('f47ac10b-58cc-4372-a567-0e02b2c3d479', 'Build Number', 'The build number for the Paper release. Leave at latest for the latest build.', 'BUILD_NUMBER', 'latest', 'required|string|max:20', true, true, 3),
    ('f47ac10b-58cc-4372-a567-0e02b2c3d479', 'Download Path', 'A URL to use to download a server.jar rather than using the PaperMC API.', 'DL_PATH', '', 'nullable|string', false, false, 4)
ON CONFLICT (flake_id, env_variable) DO NOTHING;

-- Insert Vanilla Minecraft flake
INSERT INTO flakes (id, name, slug, author, description, docker_image, startup_command, config_files, startup_detection, install_script, features)
VALUES (
    'a1b2c3d4-58cc-4372-a567-0e02b2c3d480',
    'Vanilla Minecraft',
    'vanilla',
    'Raptor Panel',
    'The official Minecraft server from Mojang.',
    'artifacts.lstan.eu/java:21',
    'java -Xms128M -Xmx{{SERVER_MEMORY}}M -jar {{SERVER_JARFILE}} nogui',
    '{
        "server.properties": {
            "parser": "properties",
            "find": {
                "server-ip": "0.0.0.0",
                "server-port": "{{server.build.default.port}}",
                "query.port": "{{server.build.default.port}}"
            }
        }
    }',
    ')! For help, type ',
    '#!/bin/bash
cd /home/container

LATEST_VERSION=$(curl -s https://launchermeta.mojang.com/mc/game/version_manifest.json | jq -r ''.latest.release'')

if [ "${MINECRAFT_VERSION}" == "latest" ]; then
    MINECRAFT_VERSION=${LATEST_VERSION}
fi

echo "Downloading Minecraft ${MINECRAFT_VERSION}"

MANIFEST_URL=$(curl -s https://launchermeta.mojang.com/mc/game/version_manifest.json | jq -r --arg VERSION "${MINECRAFT_VERSION}" ''.versions[] | select(.id == $VERSION) | .url'')

if [ -z "${MANIFEST_URL}" ]; then
    echo "Version ${MINECRAFT_VERSION} not found, using latest"
    MANIFEST_URL=$(curl -s https://launchermeta.mojang.com/mc/game/version_manifest.json | jq -r --arg VERSION "${LATEST_VERSION}" ''.versions[] | select(.id == $VERSION) | .url'')
fi

SERVER_URL=$(curl -s ${MANIFEST_URL} | jq -r ''.downloads.server.url'')

if [ -f ${SERVER_JARFILE} ]; then
    mv ${SERVER_JARFILE} ${SERVER_JARFILE}.old
fi

curl -o ${SERVER_JARFILE} ${SERVER_URL}

# Create eula.txt with eula=true
echo "eula=true" > eula.txt
echo "EULA accepted automatically"

if [ ! -f server.properties ]; then
    echo "Creating default server.properties"
    cat > server.properties << EOF
server-port=25565
query.port=25565
server-ip=0.0.0.0
EOF
fi',
    '["eula", "java_version"]'
) ON CONFLICT (slug) DO NOTHING;

-- Insert Vanilla flake variables
INSERT INTO flake_variables (flake_id, name, description, env_variable, default_value, rules, user_viewable, user_editable, sort_order)
VALUES
    ('a1b2c3d4-58cc-4372-a567-0e02b2c3d480', 'Server Memory', 'The maximum amount of memory (MB) the server can use.', 'SERVER_MEMORY', '1024', 'required|numeric|min:128', true, true, 0),
    ('a1b2c3d4-58cc-4372-a567-0e02b2c3d480', 'Minecraft Version', 'The version of Minecraft to download. Use "latest" for the latest release.', 'MINECRAFT_VERSION', 'latest', 'required|string|max:20', true, true, 1),
    ('a1b2c3d4-58cc-4372-a567-0e02b2c3d480', 'Server Jar File', 'The name of the server jarfile.', 'SERVER_JARFILE', 'server.jar', 'required|string|max:100', true, true, 2)
ON CONFLICT (flake_id, env_variable) DO NOTHING;
