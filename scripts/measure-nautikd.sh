#!/usr/bin/env bash
# Mesure la consommation CPU et mémoire de nautikd sous charge AIS (Linux,
# Raspberry Pi compris).
#
# Compile nautikd et le multiplexeur simulé (exemple nmea_feed) en mode
# release, lance les deux sur la boucle locale avec un dossier de données
# temporaire, branche nautikd sur le flux NMEA via l'API, puis relève
# /proc toutes les 2 secondes. Le dossier temporaire est supprimé à la fin.
#
# Usage : scripts/measure-nautikd.sh [cibles] [secondes]
# Dépendances : cargo, curl.
set -euo pipefail

TARGETS="${1:-1000}"
SECONDS_TOTAL="${2:-120}"
API_PORT="${API_PORT:-8391}"
FEED_PORT="${FEED_PORT:-10110}"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
cargo build --release --locked -p nautik-daemon --bin nautikd -p nav-engine --example nmea_feed

DATA="$(mktemp -d -t nautik-mesure-XXXXXX)"
BASE="http://127.0.0.1:${API_PORT}"
FEED_PID=""
DAEMON_PID=""

cleanup() {
  for pid in "$DAEMON_PID" "$FEED_PID"; do
    [ -n "$pid" ] && kill -9 "$pid" 2>/dev/null || true
  done
  rm -rf "$DATA"
}
trap cleanup EXIT

target/release/examples/nmea_feed --port "$FEED_PORT" --targets "$TARGETS" >/dev/null &
FEED_PID=$!
START_NS=$(date +%s%N)
RUST_LOG=warn target/release/nautikd --data-dir "$DATA" --listen "127.0.0.1:${API_PORT}" &
DAEMON_PID=$!

for _ in $(seq 1 100); do
  curl -fsS "$BASE/health" >/dev/null 2>&1 && break
  sleep 0.2
done
curl -fsS "$BASE/health" >/dev/null || { echo "nautikd ne répond pas" >&2; exit 1; }
STARTUP_MS=$(( ($(date +%s%N) - START_NS) / 1000000 ))

TOKEN="$(tr -d '[:space:]' < "$DATA/api-token")"
curl -fsS -X PUT "$BASE/api/v1/sources" \
  -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d "{\"mode\":\"nmea0183\",\"nmea\":{\"kind\":\"tcp\",\"host\":\"127.0.0.1\",\"port\":${FEED_PORT}}}" \
  >/dev/null

echo "Mesure : ${TARGETS} cibles AIS pendant ${SECONDS_TOTAL} s (échauffement 10 s)..."
sleep 10
TICKS=$(getconf CLK_TCK)
cpu_ticks() { awk '{print $14 + $15}' "/proc/$DAEMON_PID/stat"; }
rss_kib() { awk '/^VmRSS/ {print $2}' "/proc/$DAEMON_PID/status"; }

LAST_CPU=$(cpu_ticks)
LAST_NS=$(date +%s%N)
END=$(( $(date +%s) + SECONDS_TOTAL ))
CPU_SUM=0; CPU_MAX=0; N=0; RSS_MIN=0; RSS_MAX=0
while [ "$(date +%s)" -lt "$END" ]; do
  sleep 2
  NOW_NS=$(date +%s%N)
  CPU=$(cpu_ticks)
  # Pourcentage d'un cœur, en dixièmes.
  PCT=$(( (CPU - LAST_CPU) * 1000 * 1000000000 / TICKS / (NOW_NS - LAST_NS) ))
  RSS=$(rss_kib)
  CPU_SUM=$(( CPU_SUM + PCT )); N=$(( N + 1 ))
  [ "$PCT" -gt "$CPU_MAX" ] && CPU_MAX=$PCT
  { [ "$RSS_MIN" -eq 0 ] || [ "$RSS" -lt "$RSS_MIN" ]; } && RSS_MIN=$RSS
  [ "$RSS" -gt "$RSS_MAX" ] && RSS_MAX=$RSS
  LAST_CPU=$CPU; LAST_NS=$NOW_NS
done

STATE="$(curl -fsS "$BASE/api/v1/state" -H "Authorization: Bearer $TOKEN")"
AIS=$(printf '%s' "$STATE" | grep -o '"mmsi":' | wc -l)

echo
echo "Machine          : $(nproc) cœurs, $(uname -srm)"
echo "Cibles suivies   : ${AIS} / ${TARGETS}"
echo "Démarrage        : ${STARTUP_MS} ms"
printf 'CPU (1 cœur)     : moyenne %d.%d %%, pic %d.%d %%\n' \
  $(( CPU_SUM / N / 10 )) $(( CPU_SUM / N % 10 )) $(( CPU_MAX / 10 )) $(( CPU_MAX % 10 ))
echo "RAM (RSS)        : $(( RSS_MIN / 1024 )) à $(( RSS_MAX / 1024 )) Mio"
