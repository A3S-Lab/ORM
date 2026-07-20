#!/usr/bin/env bash
set -euo pipefail

tls_dir="${RUNNER_TEMP:?RUNNER_TEMP is required}/a3s-orm-postgres-tls"
server_dir="${tls_dir}/server"
mkdir -p "${server_dir}"

openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout "${tls_dir}/ca.key" \
  -out "${tls_dir}/ca.crt" \
  -days 1 \
  -subj "/CN=A3S ORM Test CA" >/dev/null 2>&1
openssl req -new -newkey rsa:2048 -nodes \
  -keyout "${server_dir}/server.key" \
  -out "${tls_dir}/server.csr" \
  -subj "/CN=localhost" >/dev/null 2>&1
openssl x509 -req \
  -in "${tls_dir}/server.csr" \
  -CA "${tls_dir}/ca.crt" \
  -CAkey "${tls_dir}/ca.key" \
  -CAcreateserial \
  -out "${server_dir}/server.crt" \
  -days 1 \
  -extfile <(printf "subjectAltName=DNS:localhost,IP:127.0.0.1") >/dev/null 2>&1

chmod 600 "${server_dir}/server.key"
sudo chown 70:70 "${server_dir}/server.key" "${server_dir}/server.crt"
docker run -d --name a3s-orm-postgres-tls \
  -e POSTGRES_PASSWORD=postgres \
  -e POSTGRES_DB=a3s_orm \
  -p 127.0.0.1:5433:5432 \
  -v "${server_dir}:/tls:ro" \
  --health-cmd "pg_isready -U postgres -d a3s_orm" \
  --health-interval 1s \
  --health-timeout 3s \
  --health-retries 30 \
  postgres:17-alpine \
  -c ssl=on \
  -c ssl_cert_file=/tls/server.crt \
  -c ssl_key_file=/tls/server.key

for _ in $(seq 1 45); do
  if [[ "$(docker inspect -f '{{.State.Health.Status}}' a3s-orm-postgres-tls)" == "healthy" ]]; then
    exit 0
  fi
  sleep 1
done

docker logs a3s-orm-postgres-tls
exit 1
