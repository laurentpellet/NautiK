# nautikd — service de navigation NautiK

`nautikd` fait tourner le moteur NautiK **sans écran**, en permanence :
acquisition (Signal K, NMEA 0183, simulateur), navigation, traces, AIS et
alarmes (mouillage, perte de position). Les écrans du bord s'y connectent
par une API locale protégée par un jeton.

Fermer ou éteindre un écran n'arrête jamais la surveillance : les alarmes
sont évaluées dans le service.

> NautiK n'est pas un ECDIS certifié. Il complète une veille et des moyens
> de navigation réglementaires, il ne les remplace pas.

## Plates-formes

- Linux ARM64 (Raspberry Pi 4/5, Raspberry Pi OS 64 bits, OpenPlotter) :
  cible principale, avec systemd.
- Linux x86-64, Windows, macOS : fonctionne au premier plan (sans
  systemd ni chien de garde), pratique pour les essais.

## Compilation

Sur le Raspberry Pi lui-même (Rust stable installé avec rustup) :

```bash
git clone https://github.com/laurentpellet/NautiK.git
cd NautiK
cargo build --release --locked -p nautik-daemon
```

Le binaire est `target/release/nautikd`. La CI produit aussi un binaire
Linux ARM64 (artefact `nautikd-linux-arm64` du job « Service ARM64 »).

## Paquet Debian (recommandé)

Le workflow « Paquets » produit `nautikd_<version>_arm64.deb` (Raspberry
Pi 4/5, 64 bits) et `nautikd_<version>_amd64.deb`. Pour le construire
soi-même après la compilation :

```bash
bash apps/daemon/packaging/build-deb.sh target/release/nautikd arm64 dist
sudo apt install ./dist/nautikd_*_arm64.deb
```

Le paquet installe `/usr/bin/nautikd` et l'unité systemd, crée le compte
de service `nautik` (membre de `dialout`), puis active et démarre le
service. L'API n'écoute que sur la machine. La désinstallation, même avec
`purge`, conserve `/var/lib/nautik` (traces, routes, jeton).

## Installation manuelle (systemd)

```bash
# Compte de service sans connexion, membre du groupe des ports série.
sudo useradd --system --no-create-home --shell /usr/sbin/nologin --groups dialout nautik

sudo install -m 0755 target/release/nautikd /usr/local/bin/nautikd
sudo install -m 0644 apps/daemon/systemd/nautikd.service /etc/systemd/system/nautikd.service
sudo systemctl daemon-reload
sudo systemctl enable --now nautikd
```

Vérifier :

```bash
systemctl status nautikd
journalctl -u nautikd -f
curl -s http://127.0.0.1:8375/health
```

`/health` répond `200 {"status":"ok"}` tant que la boucle moteur tourne,
`503 {"status":"stalled"}` sinon.

## Données

Dans `/var/lib/nautik` (créé par systemd, droits `0700`) :

| Fichier       | Contenu                                         |
|---------------|-------------------------------------------------|
| `nautik.db`   | Base SQLite : routes, traces, réglages, alarmes |
| `api-token`   | Jeton d'accès à l'API (`0600`)                  |
| `secrets.json`| Jeton Signal K éventuel (`0600`)                |

À sauvegarder : `nautik.db` (service arrêté, ou `sqlite3 nautik.db
".backup copie.db"`).

## Jeton d'accès

Créé au premier démarrage. Il n'est jamais écrit dans les journaux :

```bash
sudo cat /var/lib/nautik/api-token
```

Pour le révoquer et en créer un nouveau :

```bash
sudo -u nautik nautikd --rotate-token --data-dir /var/lib/nautik
sudo systemctl restart nautikd
```

## Accès depuis le réseau du bord

Par défaut l'API n'écoute que sur la machine (`127.0.0.1:8375`). Pour
l'ouvrir aux tablettes et ordinateurs du bord :

```bash
sudo systemctl edit nautikd
```

```ini
[Service]
Environment=NAUTIK_LISTEN=0.0.0.0:8375
```

L'API circule en clair (HTTP) : réservez-la au réseau du bord. Ne la
publiez jamais sur Internet ; pour un accès à distance, passez par un VPN
(WireGuard, Tailscale…).

## API

Toutes les routes sous `/api/v1` exigent l'en-tête
`Authorization: Bearer <jeton>`. Les erreurs sont `{"error": "message"}`.

```bash
TOKEN=$(sudo cat /var/lib/nautik/api-token)
curl -s -H "Authorization: Bearer $TOKEN" http://127.0.0.1:8375/api/v1/alarms
```

| Méthode et chemin                         | Rôle                                  |
|-------------------------------------------|---------------------------------------|
| `GET /api/v1/info`                        | Version du moteur et du protocole     |
| `GET /api/v1/state`                       | État complet (mêmes messages que le WebSocket) |
| `GET /api/v1/alarms`                      | État des alarmes                      |
| `POST /api/v1/alarms/anchor`              | Mouiller ici `{"radiusM":50,"delayS":10}` |
| `DELETE /api/v1/alarms/anchor`            | Lever l'ancre                         |
| `POST /api/v1/alarms/{kind}/acknowledge`  | Acquitter (`anchor`, `positionLost`)  |
| `PUT /api/v1/alarms/positionLost/enabled` | `{"enabled":false}` (historisé)       |
| `GET /api/v1/alarms/events?limit=50`      | Historique des alarmes                |
| `GET/PUT/DELETE /api/v1/navigation`       | Route active (`{"routeId":"…"}`)      |
| `GET/POST /api/v1/routes`                 | Routes (`{"name","positions"}`)       |
| `POST /api/v1/routes/{id}/reverse`, `DELETE /api/v1/routes/{id}` | |
| `GET /api/v1/waypoints`, `DELETE /api/v1/waypoints/{id}` | |
| `GET /api/v1/tracks`, `GET/DELETE /api/v1/tracks/{id}` | Traces |
| `POST/DELETE /api/v1/recording`           | Démarrer / arrêter l'enregistrement   |
| `GET/PUT /api/v1/sources`                 | Source de données (le rejeu d'un fichier est refusé à distance) |
| `GET /api/v1/serial-ports`                | Ports série présents                  |
| `GET/PUT /api/v1/simulator`               | Vitesse du bateau simulé `{"speedMps":4}` |

### Flux temps réel (WebSocket)

`ws://<hôte>:8375/api/v1/ws`. Authentification par l'en-tête
`Authorization`, ou (navigateurs) par un premier message envoyé dans les
5 s :

```json
{"type": "auth", "token": "<jeton>"}
```

Le serveur envoie ensuite `{"type":"hello","data":{…version…}}`, l'état
courant, puis chaque événement : `instruments`, `navigation`,
`sourceStatus`, `rejections`, `ais`, `alarms`, sous la forme
`{"type": "...", "data": ...}`. Sans jeton valide, la session est fermée
(code 4401). Exemple avec [websocat](https://github.com/vi/websocat) :

```bash
websocat -H "Authorization: Bearer $TOKEN" ws://127.0.0.1:8375/api/v1/ws
```

## Robustesse

- **Redémarrage automatique** (`Restart=always`, sans limite).
- **Chien de garde** (`WatchdogSec=30`) : réarmé seulement si la boucle
  moteur progresse ; sinon systemd relance le service.
- **Reconnexion** des liaisons Signal K et NMEA avec attente progressive ;
  la perte de liaison est signalée (`sourceStatus`), jamais masquée, et
  aucune position n'est inventée.
- **Reprise** au démarrage : réglages, source, surveillance du mouillage ;
  une trace interrompue par une coupure est clôturée.
- **Arrêt propre** (SIGTERM) : trace en cours terminée, clients WebSocket
  prévenus (code 1001).
- **Journaux** : journald (rotation par `SystemMaxUse=` dans
  `/etc/systemd/journald.conf`).
- **Ressources** : 256 Mo de mémoire, 80 % d'un cœur, 64 tâches au plus.

## Limites actuelles

- Pas d'alarme sonore sur le service lui-même (pas de sortie audio ni de
  buzzer) : le son est joué par les écrans connectés.
- Les écrans distants se connectent par l'API ; l'interface web servie
  par `nautikd` reste à faire.
- Les cartes (tuiles MBTiles) et la météo GRIB ne sont pas encore servies
  par l'API.

## Désinstallation

```bash
sudo systemctl disable --now nautikd
sudo rm /etc/systemd/system/nautikd.service /usr/local/bin/nautikd
sudo systemctl daemon-reload
# Données (irréversible) : sudo rm -r /var/lib/nautik
```
