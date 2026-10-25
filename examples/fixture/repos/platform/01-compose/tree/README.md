# platform

How Acme Shop runs: one `docker-compose.yml` for every service and the image
version each one runs.

## Deploying a new version

Bump the service's `image:` tag in `docker-compose.yml`, commit, and run
`docker compose up -d <service>`. Platform releases are tagged by date
(`YYYY.MM.N`).
