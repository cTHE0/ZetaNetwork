#!/bin/bash
# Script de déploiement Zeta Network

set -e

# Charger les credentials depuis un fichier séparé
CREDENTIALS_FILE="$(dirname "$0")/.deploy_credentials"

if [ ! -f "$CREDENTIALS_FILE" ]; then
    echo "ERREUR: Fichier de credentials manquant!"
    echo "Copier .deploy_credentials.example vers .deploy_credentials et remplir les mots de passe"
    exit 1
fi

# shellcheck source=/dev/null
source "$CREDENTIALS_FILE"

# Vérifier que les credentials sont définis
if [ -z "$RELAY_A_PASSWORD" ] || [ -z "$RELAY_T_PASSWORD" ]; then
    echo "ERREUR: Les mots de passe ne sont pas définis dans $CREDENTIALS_FILE"
    exit 1
fi

echo "=== Building release ==="
cargo build --release

BINARY="target/release/zeta9"

echo "=== Deploying to relayA (HubRelay) ==="
sshpass -p "$RELAY_A_PASSWORD" ssh -o StrictHostKeyChecking=no "${RELAY_A_USER}@${RELAY_A_HOST}" 'pkill -9 zeta9 || true'
sshpass -p "$RELAY_A_PASSWORD" scp "$BINARY" "${RELAY_A_USER}@${RELAY_A_HOST}":~/zeta9
sshpass -p "$RELAY_A_PASSWORD" ssh -o StrictHostKeyChecking=no "${RELAY_A_USER}@${RELAY_A_HOST}" 'chmod +x ~/zeta9'

echo "=== Deploying to relayT (Client) ==="
sshpass -p "$RELAY_T_PASSWORD" ssh -o StrictHostKeyChecking=no "${RELAY_T_USER}@${RELAY_T_HOST}" 'pkill -9 zeta9 || true'
sshpass -p "$RELAY_T_PASSWORD" scp "$BINARY" "${RELAY_T_USER}@${RELAY_T_HOST}":~/zeta9
sshpass -p "$RELAY_T_PASSWORD" ssh -o StrictHostKeyChecking=no "${RELAY_T_USER}@${RELAY_T_HOST}" 'chmod +x ~/zeta9'

echo "=== Starting HubRelay on relayA ==="
sshpass -p "$RELAY_A_PASSWORD" ssh -o StrictHostKeyChecking=no "${RELAY_A_USER}@${RELAY_A_HOST}" 'nohup ~/zeta9 --mode hub-relay --peer-id hubRelay > ~/zeta_hub.log 2>&1 &'
sleep 2

echo "=== Starting Client on relayT ==="
sshpass -p "$RELAY_T_PASSWORD" ssh -o StrictHostKeyChecking=no "${RELAY_T_USER}@${RELAY_T_HOST}" 'nohup ~/zeta9 --mode client --peer-id alice > ~/zeta_client.log 2>&1 &'
sleep 3

echo "=== Checking status ==="
echo "--- HubRelay log ---"
sshpass -p "$RELAY_A_PASSWORD" ssh -o StrictHostKeyChecking=no "${RELAY_A_USER}@${RELAY_A_HOST}" 'cat ~/zeta_hub.log'

echo ""
echo "--- Client log ---"
sshpass -p "$RELAY_T_PASSWORD" ssh -o StrictHostKeyChecking=no "${RELAY_T_USER}@${RELAY_T_HOST}" 'cat ~/zeta_client.log'

echo ""
echo "=== Deployment complete ==="
echo "HubRelay: ${RELAY_A_HOST}:55555"
echo "Client Alice: ${RELAY_T_HOST} (web: http://localhost:8080 via SSH tunnel)"
