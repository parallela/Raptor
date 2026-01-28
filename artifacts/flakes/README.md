sel# Raptor Flakes

Flakes are server templates for Raptor Panel (similar to Pterodactyl Eggs).

## Flake Format

```json
{
  "name": "Server Name",
  "slug": "server-slug",
  "author": "Author Name",
  "description": "Description of the server type",
  "dockerImage": "artifacts.lstan.eu/java:21",
  "startupCommand": "java -Xms128M -Xmx{{SERVER_MEMORY}}M -jar {{SERVER_JARFILE}}",
  "configFiles": {},
  "startupDetection": ")! For help, type ",
  "installScript": "#!/bin/bash\n...",
  "variables": [
    {
      "name": "Variable Name",
      "description": "Description",
      "envVariable": "ENV_VAR_NAME",
      "defaultValue": "default",
      "rules": "required|string",
      "userViewable": true,
      "userEditable": true
    }
  ]
}
```

## Fields

| Field | Description |
|-------|-------------|
| `name` | Display name of the flake |
| `slug` | URL-friendly identifier |
| `author` | Creator of the flake |
| `description` | Brief description |
| `dockerImage` | Docker image to use (use `artifacts.lstan.eu/java:21` for Java servers) |
| `startupCommand` | Command to start the server (supports `{{VARIABLE}}` substitution) |
| `configFiles` | Server config file modifications (optional) |
| `startupDetection` | String to detect when server is ready |
| `installScript` | Bash script to run on server creation |
| `variables` | Array of configurable variables |

## Variable Rules

Rules use a pipe-separated format:
- `required` - Value must be provided
- `nullable` - Value can be empty
- `string` - Must be a string
- `numeric` - Must be a number
- `min:X` - Minimum value (for numbers) or length (for strings)
- `max:X` - Maximum value or length

## Available Flakes

- **paper.json** - Paper Minecraft server (high performance Spigot fork)
- **vanilla.json** - Official Minecraft server from Mojang

## Importing into Raptor

1. Go to Admin → Flakes
2. Click "Create Flake"
3. Fill in the details or use "Import Egg" to import a Pterodactyl egg

Note: When importing Pterodactyl eggs, the docker image is automatically replaced with our artifact image.
