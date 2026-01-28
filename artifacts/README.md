# Raptor Artifacts

This directory contains Docker images for running game servers.

## Java Runtime

The `java/` directory contains a Docker image for running Java-based game servers like Minecraft.

### Building

```bash
cd artifacts/java
docker build --platform linux/amd64 -t artifacts.lstan.eu/java:21 .
```

### Pushing

```bash
docker push artifacts.lstan.eu/java:21
```

### Usage

The container expects:
- Server files mounted at `/home/container`
- `STARTUP` environment variable with the startup command

Example:
```bash
docker run -d \
  --name minecraft-server \
  -v /path/to/server:/home/container \
  -e STARTUP="java -Xms1G -Xmx4G -jar server.jar nogui" \
  -p 25565:25565 \
  artifacts.lstan.eu/java:21
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `STARTUP` | The command to run the server | `java -Xms128M -Xmx1024M -jar server.jar` |
| `TZ` | Timezone | `UTC` |

### Variable Substitution

The startup command supports `{{VARIABLE}}` syntax which will be replaced with environment variable values:

```bash
-e STARTUP="java -Xms{{MIN_MEMORY}} -Xmx{{MAX_MEMORY}} -jar server.jar" \
-e MIN_MEMORY="1G" \
-e MAX_MEMORY="4G"
```

### Signal Handling

The entrypoint uses `exec` to run the server process directly, which means:
- SIGTERM is sent directly to the Java process
- The server can handle graceful shutdown properly
- This works with Docker's stop command and the Raptor panel's stop button
