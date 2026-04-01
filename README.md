# 🌐 Zeta Network - Réseau Social Décentralisé

> Un réseau social peer-to-peer décentralisé avec propagation intelligente et signatures cryptographiques Ed25519.

[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## 📖 Table des matières

- [Vue d'ensemble](#vue-densemble)
- [Architecture](#architecture)
- [Installation](#installation)
- [Utilisation rapide](#utilisation-rapide)
- [Guide complet pour débutants](#guide-complet-pour-débutants)
  - [Lancer le HubRelay](#1-lancer-le-hubrelay)
  - [Lancer un Client](#2-lancer-un-client)
  - [Utiliser l'interface web](#3-utiliser-linterface-web)
- [Détails techniques](#détails-techniques)
- [Dépannage](#dépannage)

---

## 🎯 Vue d'ensemble

**Zeta Network** est un réseau social **100% décentralisé** qui permet aux utilisateurs de :
- ✅ **Publier des messages** signés cryptographiquement (impossible à falsifier)
- ✅ **S'abonner à d'autres utilisateurs** et recevoir leurs messages
- ✅ **Participer au réseau** comme **Client** ou **Relay**
- ✅ **Pas de serveur central** (sauf l'annuaire HubRelay pour la découverte)

### Différence avec les réseaux sociaux classiques

| Aspect | Zeta Network | Facebook / Twitter |
|--------|-------------|-------------------|
| **Données** | Stockées localement chez vous | Chez la compagnie |
| **Propriété** | Vos messages vous appartiennent | Facebook les possède |
| **Censure** | Impossible (vous pouvez tout stocker) | Oui |
| **Vie privée** | Clés Ed25519 locales | Données vendues aux annonceurs |
| **Décentralisation** | 100% P2P | Centralisé (1 serveur) |

---

## 🏗️ Architecture

### Comment fonctionne Zeta Network

```
                    ┌─────────────────────┐
                    │    HUBRELAY         │
                    │ (Annuaire central)  │
                    │ 65.75.200.180:55555 │
                    └─────────────────────┘
                     (Stocke la liste des nœuds)
                              △
                              │ NodeAnnounce (60s)
                ┌─────────────┼──────────────┐
                │             │              │
             RELAY          RELAY          CLIENT
          (NAT ouverte)  (NAT ouverte)  (NAT restrictive)
               △◄────────────────────────────┐
               │ PublishPost                 │
               │ PostsBatch                  │ RequestPosts
               │                             │
               └─────────────────────────────┘
```

### Les 3 types de nœuds

#### 🔄 **RELAY** (Relais)
- **Rôle** : Reçoit et transmet les messages
- **NAT** : Ouverte (OpenInternet, FullCone, RestrictedCone, etc.)
- **Qualités** :
  - ✅ Peut recevoir des connexions entrantes
  - ✅ Stocke les posts
  - ✅ **Propage les posts** à d'autres relays et clients
  - ✅ Répond aux demandes des clients (RequestPosts)
- **exemple** : Un serveur avec IP publique fixe

#### 📱 **CLIENT** (Client)
- **Rôle** : Envoie et reçoit les messages
- **NAT** : Restrictif (Symmetric, UdpBlocked, etc.)
- **Qualités** :
  - ✅ Peut publier des posts
  - ✅ Peut s'abonner et recevoir les posts
  - ❌ **Ne propage PAS** les posts aux autres
  - ❌ **Ne répond PAS** aux demandes des autres clients
  - ❌ Dépend entièrement des relays
- **exemple** : Votre ordinateur derrière une box internet

#### 📋 **HUBRELAY** (Annuaire)
- **Rôle** : Annuaire centralisé (seulement pour la découverte)
- **Stocke** : Liste des nœuds actifs (IP, clé publique, type)
- **Ne propage PAS** les messages (c'est important !)
- **Répond à** :
  - `NodeAnnounce` → Enregistre un nœud
  - `GetAllNodes` → Renvoie la liste complète

### Comment un post circule dans le réseau

```
1. CRÉATION
   Alice (client) crée un post "Hello Zeta!"
   ↓
   Post signé avec sa clé Ed25519 privée
   ↓

2. ENVOI
   Alice envoie le post aux RELAYS uniquement
   (elle ne peut rien envoyer d'autre que ça)
   ↓

3. PROPAGATION par Relay 1
   Relay 1 reçoit le post
   ✅ Vérifie la signature (ok)
   ✅ Le stocke dans sa base SQLite
   ✅ Le propage aux autres RELAYS
   ✅ Le propage à tous ses CLIENTS connectés
   ↓

4. ARRIVÉE chez Bob (client)
   Bob reçoit le post d'Alice via un relay
   ✅ Vérifie la signature (ok, c'est vraiment Alice)
   ✅ Le stocke dans sa base SQLite
   ✅ L'affiche dans son fil d'actualité si c'est un abonnement
```

**Points clés** :
- ✅ Les clients ne propagent **JAMAIS** (évite les boucles)
- ✅ Seuls les relays propagent
- ✅ Chaque post est signé (impossible à falsifier)
- ✅ Protection anti-boucle avec `seen_posts`

---

## 📦 Installation

### Prérequis

- **Rust 1.70+** : [Installer Rust ici](https://rustup.rs/)
- **Git**
- **Connexion UDP** (plus important : pas de firewall UDP)

### Télécharger et compiler

```bash
# Cloner le dépôt
git clone <votre-url>
cd zeta_network

# Compiler (version optimisée)
cargo build --release

# Vérifier que tout compile
cargo check
```

---

## 🚀 Utilisation rapide

### En 3 étapes

#### 1️⃣ Lancer le HubRelay (serveur)
```bash
cargo run --release -- --mode hub-relay --peer-id hub1
```

#### 2️⃣ Lancer un client
```bash
cargo run --release -- --mode client --peer-id alice
```

#### 3️⃣ Ouvrir l'interface web
```
http://localhost:8080
```

---

## 📚 Guide complet pour débutants

### 1️⃣ Lancer le HubRelay

#### Qu'est-ce que le HubRelay ?

Le **HubRelay** est l'**annuaire central** du réseau. C'est par là que tout commence.

**Analogie** : Si Zeta Network est une ville, le HubRelay est la mairie.
- Les citoyens (nœuds) vont à la mairie s'enregistrer
- La mairie garde une liste de tous les habitants
- Les habitants peuvent demander l'adresse des autres habitants

#### Configuration

**IMPORTANT** : Le HubRelay doit avoir une **IP publique fixe**.

Actuellement, c'est codé en dur dans le code :
```rust
// src/main.rs:20
let hub_relay_addr: SocketAddr = "65.75.200.180:55555".parse().unwrap();
```

**Si vous avez une IP publique différente** :
1. Éditez `src/main.rs` ligne 20
2. Remplacez `65.75.200.180` par votre IP
3. Recompilez : `cargo build --release`

#### Lancement

```bash
cargo run --release -- --mode hub-relay --peer-id hub1
```

**Sortie attendue** :
```
=== HubRelay : Serveur d'annuaire du réseau social ===
Écoute sur 65.75.200.180:55555 (ID: hub1)
Prêt à recevoir des nœuds...

[NodeAnnounce] 192.168.1.100:54321 (03abc123...) [relay] (14:32:10)
  → Nœud enregistré / mis à jour

[GetAllNodes] 192.168.1.100:54321 (14:32:12)
  → Envoi de la liste complète : 5 nœuds

[STATS] 5 nœuds actifs → 2 relay(s) + 3 client(s)
```

#### Fonctionnement

Le HubRelay exécute 3 services en boucle :

```
Service 1 : Réception des messages (continu)
  - NodeAnnounce → Enregistre/met à jour un nœud
  - GetAllNodes → Renvoie la liste complète

Service 2 : Nettoyage (toutes les 30s)
  - Supprime les nœuds inactifs depuis > 5 minutes

Service 3 : Statistiques (toutes les 60s)
  - Affiche le nombre de relays et de clients
```

**⚠️ Important** :
- Le HubRelay **ne propage PAS** les messages (posts)
- Il est **uniquement un annuaire** (comme un DNS)
- Les nœuds communiquent directement entre eux, pas via le HubRelay

---

### 2️⃣ Lancer un Client

#### Qu'est-ce qu'un Client ?

Un **Client** est un nœud qui participe au réseau. À son lancement, il :
1. Détecte **automatiquement** son type de NAT
2. Génère sa **clé Ed25519** (identité unique)
3. S'**enregistre** au HubRelay
4. Télécharge la **liste des nœuds**
5. Lance son **serveur web** http://localhost:8080
6. Se connecte à des **relays** pour envoyer/recevoir des messages

#### Détection du rôle

Le client choisit **automatiquement** son rôle selon son NAT :

```
NAT ouverte ?
  → Oui : OpenInternet / FullCone / RestrictedCone / PortRestrictedCone
     ↓ RELAY (peut recevoir les connexions)
  → Non : Symmetric / UdpBlocked / Unknown
     ↓ CLIENT (dépend des relays)
```

#### Lancement

```bash
cargo run --release -- --mode client --peer-id alice
```

(Remplacez `alice` par votre nom d'utilisateur)

#### Sortie si RELAY

```
=== Initialisation du nœud ===
Détection du type de NAT...
  → Type NAT : OpenInternet
  → Clé publique : 03abc123def456...
  → Adresse publique : 192.168.1.100:54321

[RÔLE] RELAY
  → Vous avez une NAT ouverte
  → Vous recevrez les posts de TOUS les relays
  → Vous propagerez les posts à VOS clients et autres relays
  → Les autres nœuds peuvent vous envoyer des demandes

Enregistrement auprès du HubRelay...
  → ✅ Enregistré avec succès

Récupération de la liste des nœuds...
  → ✅ 5 nœuds trouvés (2 relays, 3 clients)

Démarrage des services...
  → Service web : http://localhost:8080
[NET] Prêt à recevoir des messages UDP
```

#### Sortie si CLIENT

```
=== Initialisation du nœud ===
Détection du type de NAT...
  → Type NAT : Symmetric
  → Clé publique : 03def456abc789...
  → Adresse publique : 10.0.0.50:49152

[RÔLE] CLIENT
  → Vous avez une NAT restrictive
  → Vous NE recevrez les posts que via les relays
  → Vous NE propagerez PAS les posts
  → Vous NE répondrez PAS aux demandes

Enregistrement auprès du HubRelay...
  → ✅ Enregistré avec succès

Récupération de la liste des nœuds...
  → ✅ 5 nœuds trouvés (2 relays, 3 clients)

Démarrage des services...
  → Service web : http://localhost:8080
[NET] Prêt à recevoir les messages des relays
```

#### Services en arrière-plan

Une fois lancé, le client exécute **6 services automatiquement** :

| # | Service | Fréquence | Description |
|---|---------|-----------|-------------|
| 1 | **Web Server** | Continu | Interface http://localhost:8080 |
| 2 | **Keep-alive HubRelay** | 60s | Envoie `NodeAnnounce` pour rester actif |
| 3 | **Mise à jour nœuds** | 30s | Demande `GetAllNodes` au HubRelay |
| 4 | **Synchronisation posts** | 30s | Demande posts récents des abonnements |
| 5 | **Annonce aux pairs** | 60s | Envoie `NodeAnnounce` aux peers |
| 6 | **Nettoyage peers** | 60s | Supprime les peers inactifs > 5 min |

**Détail du Service 4 (Synchronisation posts)** :
```
Toutes les 30 secondes :
  1. Récupère vos abonnements depuis SQLite
  2. Pour chaque relay connu :
     - Envoie : "Donne-moi les posts de [Alice, Bob, Charlie] depuis le timestamp X"
     - Reçoit : PostsBatch avec jusqu'à 100 posts
     - Stocke : Chaque post dans SQLite (après vérification de signature)
  3. Le Service 1 (Web) affiche immédiatement les nouveaux posts
```

---

### 3️⃣ Utiliser l'interface web

#### Ouverture

Une fois le client lancé, ouvrez votre navigateur :
```
http://localhost:8080
```

#### Interface utilisateur

L'interface comporte **4 onglets** :

---

#### 📝 **Onglet "Fil d'actualité"**

**Haut de page :**
```
Votre clé publique : 03abc123def456...789 [Copier]
```
- C'est votre **identité unique** dans le réseau
- Donnez-la à d'autres pour qu'ils vous suivent

**Zone de publication :**
```
┌─────────────────────────────────────┐
│ Quoi de neuf ?                      │
│                                     │
│ [Écrivez votre message ici...]      │
│                                     │
│ 0 / 500 caractères          [Publier]
└─────────────────────────────────────┘
```
- Max 500 caractères
- Le compteur se met à jour en temps réel
- Clic sur "Publier"
  - ✅ Post signé Ed25519
  - ✅ Envoyé à tous les relays
  - ✅ Stocké dans votre base de données
  - ✅ Apparaît immédiatement dans le fil

**Fil d'actualité :**
```
┌──────────────────────────────────────┐
│ Alice (03abc123...)   14:32:10       │
│ ✓                                    │
│ Bonjour! C'est mon premier post      │
│                                      │
├──────────────────────────────────────┤
│ Bob (03def456...)     14:33:45       │
│ ✓                                    │
│ Super ça fonctionne !                │
└──────────────────────────────────────┘
```

- ✓ = Signature valide (vert)
- ⚠ = Signature invalide (rouge) - Post falsifié ?

**Auto-refresh** : Toutes les **3 secondes**

---

#### 👥 **Onglet "Abonnements"**

**Zone d'ajout :**
```
Ajouter un abonnement

Clé publique : [03abc123def456...]
               [Ajouter]
```
- Collez la clé publique complète (66 caractères en hex)
- Clic sur "Ajouter"
- ✅ Stocké dans SQLite
- ✅ Service 4 commencera à demander ses posts

**Liste :**
```
Vos abonnements (2):

03abc123... (Alice)     [X]
03def456... (Bob)       [X]

X = Clic pour se désabonner
```

---

#### 🌐 **Onglet "Réseau"**

**Statistiques :**
```
📊 Réseau Zeta

Nœuds actifs         : 5
  └─ Relays         : 2 🔄
  └─ Clients        : 3 📱
```

**Liste des nœuds :**
```
┌────────────────────────────────────┐
│ 03abc123...  192.168.1.100:54321   │
│ 🔄 RELAY     Last seen: 5s ago     │
│                        [Suivre]    │
├────────────────────────────────────┤
│ 03def456...  10.0.0.50:49152       │
│ 📱 CLIENT    Last seen: 2s ago     │
│                        [Suivre]    │
└────────────────────────────────────┘

Bouton [Suivre] → Ajoute directement aux abonnements
```

---

#### 🔗 **Onglet "Peers"**

```
📡 Peers connectés (0)

(Vous êtes un client, vous transitez par les relays)
```

**Pour les relays** :
```
📡 Peers connectés (3)

192.168.1.100:45678  03abc... Last: 2s
10.0.0.50:49152      03def... Last: 5s
172.16.0.1:54000     03ghi... Last: 10s
```

---

### Scénario complet : Publier et recevoir des messages

#### 🎬 Étape par étape

**1. Alice lance le client (relay)**
```bash
cargo run -- --mode client --peer-id alice
```
- Détecte NAT ouverte → RELAY
- Clé publique : `03abc...`

**2. Bob lance le client (client)**
```bash
cargo run -- --mode client --peer-id bob
```
(Port web : 8081 pour éviter conflit)
- Détecte NAT restrictive → CLIENT
- Clé publique : `03def...`

**3. Alice publie un post**
- Ouvre http://localhost:8080
- Écrit : "Hello from Alice!"
- Clic "Publier"
- 📤 Post envoyé à tous les relays (à lui-même et autres)
- ✅ Apparent immédiatement dans son fil

**4. Bob s'abonne à Alice**
- Ouvre http://localhost:8081
- Onglet "Abonnements"
- Entre la clé publique d'Alice : `03abc...`
- Clic "Ajouter"

**5. Attendre 30 secondes**
- Service 4 de Bob s'exécute
- Bob envoie au relay d'Alice : `RequestPosts { pubkeys: [03abc...], since: 3600 }`
- Le relay d'Alice répond : `PostsBatch { posts: [...] }`

**6. Bob voit le post d'Alice**
- Onglet "Fil d'actualité"
- Voit : "Alice - Hello from Alice!" ✓
- Auto-refresh 3s

**7. Propagation complète**
- Charlie (client) s'abonne à Alice
- Même processus
- Tous les clients abonnés voient le même post

---

## 🔧 Détails techniques

### Base de données SQLite

**Fichier** : `zeta_data.db` (créé automatiquement)

**Tables** :

```sql
-- Paire de clés Ed25519 persistante
CREATE TABLE keypair (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    secret_key TEXT NOT NULL,
    public_key TEXT NOT NULL
);

-- Posts reçus et créés
CREATE TABLE posts (
    id TEXT PRIMARY KEY,              -- Hash SHA256 du post
    author_pubkey TEXT NOT NULL,      -- Clé publique de l'auteur
    content TEXT NOT NULL,            -- Message (max 500 chars)
    signature TEXT NOT NULL,          -- Signature Ed25519 (130 hex)
    timestamp INTEGER NOT NULL        -- Unix timestamp
);

-- Utilisateurs suivis
CREATE TABLE subscriptions (
    pubkey TEXT PRIMARY KEY
);

-- Peers connus
CREATE TABLE peers (
    addr TEXT PRIMARY KEY,
    pubkey TEXT,
    last_seen INTEGER NOT NULL
);
```

### Cryptographie Ed25519

**Chaque post** dans le réseau est signé avec Ed25519 :

```rust
// Données du post
let content = "Hello Zeta!";
let timestamp = 1712076730;
let author_pubkey = "03abc123...";

// On signe le triplet
let data = format!("{}:{}:{}", content, timestamp, author_pubkey);
let signature = Ed25519_sign(data, secret_key);

// Post final
post.id = SHA256(data)[..16]        // ID unique
post.signature = signature          // 130 hex chars
```

**Vérification** :
```rust
// N'importe qui peut vérifier
let data = format!("{}:{}:{}", post.content, post.timestamp, post.author_pubkey);
assert!(Ed25519_verify(data, post.signature, post.author_pubkey));
```

**Conséquence** :
- ✅ Impossible de falsifier un post
- ✅ Impossible de voler l'identité de quelqu'un d'autre
- ✅ Tous les nœuds peuvent vérifier

### Messages P2P (Bincode encoding)

**6 types de messages** :

```rust
Message::PublishPost { post }
  → Publie un nouveau post
  → Envoyeur : N'importe quel nœud
  → Destinataire : Relays

Message::RequestPosts { src_addr, since, pubkeys }
  → Demande les posts d'auteurs spécifiques depuis un timestamp
  → Envoyeur : Clients
  → Destinataire : Relays
  → Réponse : PostsBatch

Message::PostsBatch { posts }
  → Réponse à RequestPosts avec jusqu'à 100 posts
  → Envoyeur : Relays
  → Destinataire : Le demandeur

Message::NodeAnnounce { addr, pubkey, is_relay, time }
  → Annonce d'un nœud (enregistrement)
  → Envoyeur : Tous les nœuds
  → Destinataire : HubRelay

Message::GetAllNodes { src_addr, time }
  → Demande la liste de tous les nœuds
  → Envoyeur : Tous les nœuds
  → Destinataire : HubRelay
  → Réponse : AllNodesList

Message::AllNodesList { nodes }
  → Liste de tous les nœuds actifs
  → Envoyeur : HubRelay
  → Destinataire : Le demandeur
```

### Protection anti-boucle

**Problème** : Si on propagande tous les posts à tous les nœuds, on peut avoir des boucles :
```
Relay A → Post → Relay B → Post → Relay A (BOUCLE INFINIE!)
```

**Solution** : `HashSet<String>` des IDs vus :

```rust
pub type SeenPosts = Arc<Mutex<HashSet<String>>>;

// À la réception d'un PublishPost
let is_new = seen_posts.insert(post.id.clone());
if !is_new {
    return;  // Déjà vu, ignorer (BOUCLE ARRÊTÉE)
}

// Seulement si nouveau, on vérifie la signature
if !post.verify() { return; }

// Seulement si valide, on le stocke et le propage
storage.save_post(&post);
broadcast_post(&post);  // Propage à d'autres
```

### Buffer UDP

```rust
let mut buf = vec![0; 65536];  // 64KB
```

**Calcul** :
- 1 Post sérialisé ≈ 200 bytes
- PostsBatch de 100 posts ≈ 20KB
- Marge : 64KB peut contenir jusqu'à 300 posts

**UDP limite** : 65.536 bytes (max UDP datagram)

---

## 🐛 Dépannage

### Erreur : "Failed to bind socket"

```
Error: Os { code: 48, kind: AddrInUse, message: "Address already in use" }
```

**Cause** : Le port 55555 (HubRelay) ou 8080 (Web) est déjà utilisé.

**Solutions** :

```bash
# Voir quel processus utilise le port
lsof -i :55555
lsof -i :8080

# Tuer le processus
kill -9 <PID>

# Ou attendre 60s (TIME_WAIT de TCP)
```

---

### Erreur : "Failed to get public IP"

```
[ERROR] Impossible de détecter le type de NAT
Error: Timeout contacting STUN server
```

**Cause** : Le serveur STUN est inaccessible ou UDP est bloqué.

**Solutions** :

1. **Vérifiez votre firewall**
   ```bash
   # Tester la connexion UDP
   nc -u stun.l.google.com 19302
   ```

2. **Changez le serveur STUN** dans `src/lib_p2p.rs:119`
   ```rust
   // Autres serveurs STUN publics
   "stun.services.mozilla.com:3478"
   "stun1.l.google.com:19302"
   "stun.stunprotocol.org:3478"
   ```

3. **Lancez sur un serveur avec IP publique** (VPS, cloud)

---

### Problème : "Le client ne reçoit aucun post"

**Diagnostique** :

1. **Vérifiez que le HubRelay est lancé**
   ```
   Vous devez voir : "[STATS] X nœuds actifs"
   ```

2. **Vérifiez qu'il y a des relays**
   ```
   Logs du client : "5 nœuds trouvés (2 relays)"

   Si 0 relays → Problème : Aucun nœud avec NAT ouverte
   ```

3. **Vérifiez que vous avez au moins 1 abonnement**
   ```
   Interface web → Onglet "Abonnements" → Au moins 1 clé publique
   ```

4. **Attendez 30 secondes** (Service 4 s'exécute toutes les 30s)

5. **Vérifiez les logs**
   ```bash
   # Rechercher les messages stockés
   grep "Post stocké" logs.txt
   grep "RequestPosts" logs.txt
   ```

---

### Erreur : "Ce nœud ne peut pas accéder au réseau"

```
[ERROR] Ce nœud ne peut pas accéder au réseau (NAT : UdpBlocked)
```

**Cause** : Votre réseau bloque complètement UDP.

**Solutions** :
- Désactivez temporairement le firewall
- Utilisez un VPS avec IP publique
- Utilisez un VPN (si autorisé par l'administrateur réseau)

---

### L'interface web ne charge pas

1. **Le client est bien lancé ?**
   ```
   Vous devez voir : "[WEB] Interface disponible sur http://127.0.0.1:8080"
   ```

2. **Bon URL ?**
   ```
   http://localhost:8080  (pas http://localhost:8081 ou autre)
   ```

3. **Port changé ?**
   - Modifiez `src/client.rs:14` si besoin
   - Recompilez : `cargo build --release`

---

### Pourquoi je ne vois pas les posts en temps réel ?

**C'est normal !** La synchronisation se fait toutes les **30 secondes**, pas en temps réel.

**Raison** : UDP sur internet n'est pas fiable pour les notifications push.

**Solution** : Web Socket (à implémenter en future version)

---

## 🧪 Tests

### Lancer les tests

```bash
cargo test
```

**Tests unitaires** :
- `crypto::tests::test_sign_verify` : Signatures Ed25519
- `post::tests::test_post_creation_and_verification` : Création et vérification
- `post::tests::test_tampered_post_fails_verification` : Détection de falsification
- `storage::tests::test_storage` : Stockage SQLite

### Simulation locale complète (3 terminaux)

**Terminal 1 : HubRelay**
```bash
cargo run --release -- --mode hub-relay --peer-id hub1
```

**Terminal 2 : Alice (relay potentiel)**
```bash
cargo run --release -- --mode client --peer-id alice
# http://localhost:8080
```

**Terminal 3 : Bob (relay potentiel)**
```bash
# Modifier WEB_PORT dans src/client.rs:14 pour éviter conflit
# Changer 8080 → 8081
cargo run --release -- --mode client --peer-id bob
# http://localhost:8081
```

**Scénario de test** :
1. Alice ouvre http://localhost:8080
2. Alice publie : "Hello from Alice!"
3. Bob ouvre http://localhost:8081
4. Bob va à "Réseau" et clique "Suivre" sur Alice
5. Bob attend 30 secondes
6. Bob voit le post d'Alice dans son fil ✓

---

## 📁 Structure du projet

```
zeta_network/
├── src/
│   ├── main.rs              # Point d'entrée
│   ├── client.rs            # Exécution mode client
│   ├── hub_relay.rs         # Exécution mode HubRelay
│   ├── network.rs           # Logique réseau (NetworkState)
│   ├── lib_p2p.rs           # Messages et utilitaires
│   ├── post.rs              # Structure Post + signatures
│   ├── crypto.rs            # Cryptographie Ed25519
│   ├── storage.rs           # Base SQLite
│   ├── nat_detector/        # Détection type NAT
│   │   ├── mod.rs
│   │   └── util.rs
│   └── web/
│       ├── mod.rs           # Serveur Axum
│       └── static/
│           └── index.html   # Interface web
├── Cargo.toml
├── README.md                # Ce fichier
└── ANALYSE_COMPLETE.md      # Analyse technique complète
```

---

## 🔗 Liens utiles

- [Rust Book](https://doc.rust-lang.org/book/)
- [Ed25519 Specification](https://ed25519.cr.yp.to/)
- [NAT Types & STUN](https://www.rfc-editor.org/rfc/rfc3489)
- [UDP Protocol](https://www.rfc-editor.org/rfc/rfc768)
- [SQLite Documentation](https://www.sqlite.org/docs.html)

---

## 📝 Licence

MIT License

---

**Développé avec ❤️ pour la décentralisation**

*Questions ? Ouvrez une issue sur GitHub !*
