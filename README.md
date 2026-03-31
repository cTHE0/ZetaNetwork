# Zeta Network

Réseau social décentralisé P2P écrit en Rust.

## Architecture

```
                    ┌──────────────────┐
                    │    Hub Relay     │
                    │  65.75.200.180   │
                    │    Port 55555    │
                    └────────┬─────────┘
                             │
           ┌─────────────────┼─────────────────┐
           │                 │                 │
    ┌──────▼──────┐   ┌──────▼──────┐   ┌──────▼──────┐
    │  Client A   │   │  Client B   │   │  Client C   │
    │   (Relay)   │◄──►  (Client)   │◄──►  (Client)   │
    │  Web :8080  │   │  Web :8080  │   │  Web :8080  │
    └─────────────┘   └─────────────┘   └─────────────┘
```

### Composants

| Module | Description |
|--------|-------------|
| **Hub Relay** | Serveur central de coordination. Maintient la liste des nœuds et dirige les nouveaux clients vers des relays disponibles. |
| **Client** | Nœud utilisateur. Peut devenir relay si son NAT le permet (FullCone, RestrictedCone, etc.). |
| **Interface Web** | API REST + interface HTML sur `localhost:8080` |
| **SQLite** | Stockage local des posts, abonnements et clés |

### Flux de données

1. **Création d'un post** : L'utilisateur soumet via l'interface web → signature ECDSA (secp256k1) → stockage SQLite → diffusion aux peers
2. **Réception** : Message reçu → vérification de la signature → stockage si valide
3. **Synchronisation** : Toutes les 30s, demande des posts récents aux peers (abonnements)

## Prérequis

- Rust (cargo) : https://rustup.rs/
- Connexion UDP sortante (port 55555 pour le hub, port dynamique pour les clients)

## Installation

```bash
git clone https://github.com/cTHE0/ZetaNetwork.git
cd ZetaNetwork
cargo build --release
```

Le binaire se trouve dans `target/release/zeta9`.

## Lancement

### Option 1 : Hub Relay (serveur central)

Le Hub Relay est le point d'entrée du réseau. Il doit être accessible sur une IP publique.

```bash
# Sur le VPS du Hub Relay (ex: 65.75.200.180)
./target/release/zeta9 --mode hub-relay --peer-id hubRelay
```

**En arrière-plan :**
```bash
nohup ./target/release/zeta9 --mode hub-relay --peer-id hubRelay > zeta_hub.log 2>&1 &
```

**Vérifier les logs :**
```bash
tail -f zeta_hub.log
```

**Ports requis :**
- UDP 55555 (entrant) : Communication P2P

---

### Option 2 : Client (nœud utilisateur)

Le client se connecte au Hub Relay, puis communique en P2P avec les autres nœuds.

```bash
# Sur n'importe quelle machine
./target/release/zeta9 --mode client --peer-id monPseudo
```

**En arrière-plan :**
```bash
nohup ./target/release/zeta9 --mode client --peer-id alice > zeta_client.log 2>&1 &
```

**Accéder à l'interface web :**
- Local : http://localhost:8080
- Distant (via SSH tunnel) : `ssh -L 8080:localhost:8080 user@vps-ip`

**Ports requis :**
- UDP sortant vers 65.75.200.180:55555 (Hub Relay)
- TCP 8080 (local uniquement) : Interface web

---

## Commandes par VPS

### VPS Hub Relay (65.75.200.180)

```bash
# 1. Cloner et compiler
git clone https://github.com/cTHE0/ZetaNetwork.git
cd ZetaNetwork
cargo build --release

# 2. Lancer le Hub Relay
nohup ./target/release/zeta9 --mode hub-relay --peer-id hubRelay > zeta_hub.log 2>&1 &

# 3. Vérifier
tail -f zeta_hub.log
```

### VPS Client (ex: 65.75.201.11)

```bash
# 1. Cloner et compiler
git clone https://github.com/cTHE0/ZetaNetwork.git
cd ZetaNetwork
cargo build --release

# 2. Lancer le client
nohup ./target/release/zeta9 --mode client --peer-id alice > zeta_client.log 2>&1 &

# 3. Vérifier
tail -f zeta_client.log

# 4. Accéder à l'interface web (depuis votre machine locale)
# ssh -L 8080:localhost:8080 user@65.75.201.11
# Puis ouvrir http://localhost:8080
```

---

## Rejoindre le réseau (nouveau nœud)

### En tant que Client simple

```bash
git clone https://github.com/cTHE0/ZetaNetwork.git
cd ZetaNetwork
cargo build --release
./target/release/zeta9 --mode client --peer-id votreNom
```

Le client :
1. Détecte automatiquement votre type de NAT
2. Obtient votre IP publique via STUN
3. Se connecte au Hub Relay
4. Reçoit un relay disponible
5. Démarre l'interface web sur :8080

### En tant que Relay (contribuer au réseau)

Si votre NAT est favorable (FullCone, RestrictedCone, PortRestrictedCone, OpenInternet), votre nœud se déclarera automatiquement comme relay et aidera les autres clients à se connecter.

**Conditions pour être relay :**
- IP publique accessible
- NAT favorable (détection automatique)
- Port UDP ouvert en entrée

---

## API REST

| Endpoint | Méthode | Description |
|----------|---------|-------------|
| `/` | GET | Interface web HTML |
| `/api/identity` | GET | Votre clé publique |
| `/api/posts` | GET | Posts des abonnements |
| `/api/posts` | POST | Créer un post (`{"content": "..."}`) |
| `/api/subscriptions` | GET | Liste des abonnements |
| `/api/subscriptions` | POST | S'abonner (`{"pubkey": "..."}`) |
| `/api/subscriptions/{pk}` | DELETE | Se désabonner |
| `/api/peers` | GET | Liste des peers connectés |
| `/api/network` | GET | Liste de tous les nœuds du réseau |

**Exemple avec curl :**
```bash
# Voir son identité
curl http://localhost:8080/api/identity

# Publier un post
curl -X POST http://localhost:8080/api/posts \
  -H "Content-Type: application/json" \
  -d '{"content": "Hello Zeta Network!"}'

# S'abonner à quelqu'un
curl -X POST http://localhost:8080/api/subscriptions \
  -H "Content-Type: application/json" \
  -d '{"pubkey": "02abc123..."}'

# Voir tous les nœuds du réseau
curl http://localhost:8080/api/network
```

---

## Gestion des processus

```bash
# Voir les processus Zeta
ps aux | grep zeta9

# Arrêter proprement
pkill zeta9

# Arrêter de force
pkill -9 zeta9
```

---

## Fichiers générés

| Fichier | Description |
|---------|-------------|
| `zeta_data.db` | Base SQLite (posts, abonnements, clés, peers) |
| `zeta_hub.log` | Logs du Hub Relay |
| `zeta_client.log` | Logs du client |

---

## Cryptographie

- **Courbe** : secp256k1 (identique à Bitcoin/Ethereum)
- **Signature** : ECDSA
- **Hash** : SHA-256

Chaque post est signé avec la clé privée de l'auteur. La signature est vérifiée par tous les nœuds qui reçoivent le post.

---

## Dépannage

### Le client ne démarre pas
```bash
# Vérifier que le Hub Relay est accessible
nc -vzu 65.75.200.180 55555
```

### Pas de posts reçus
- Vérifiez vos abonnements : `curl http://localhost:8080/api/subscriptions`
- Vérifiez les peers : `curl http://localhost:8080/api/peers`

### Interface web inaccessible
- Le serveur écoute uniquement sur localhost
- Utilisez un tunnel SSH : `ssh -L 8080:localhost:8080 user@vps`

---

## Licence

MIT
