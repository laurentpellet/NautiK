
# NautiK — Architecture technique

## 1. Stack

Backend :
- Rust stable
- Tokio
- Axum
- SQLx
- Serde
- Tracing

Desktop :
- Tauri 2

Frontend :
- Svelte 5
- TypeScript
- Vite
- MapLibre GL JS

Base de données :
- SQLite

## 2. Architecture

Trois composants indépendants :

1. Moteur Rust.
2. Application desktop Tauri.
3. Interface Svelte.

Le moteur doit pouvoir fonctionner sans Tauri.

## 3. Workspace

nautik/
├── apps/
│   ├── desktop/
│   └── daemon/
├── crates/
│   ├── nav-core/
│   ├── nav-geodesy/
│   ├── nav-nmea0183/
│   ├── nav-nmea2000/
│   ├── nav-signalk/
│   ├── nav-ais/
│   ├── nav-charts/
│   ├── nav-routing/
│   ├── nav-alarms/
│   ├── nav-storage/
│   └── nav-api/
├── migrations/
├── fixtures/
├── tests/
└── Cargo.toml

## 4. Responsabilités

### nav-core

- Types métier.
- État du bateau.
- Mesures normalisées.
- Qualité des données.
- Arbitrage des sources.
- Bus d'événements.

### nav-geodesy

- Distances géodésiques.
- Relèvements.
- Conversions d'unités.
- Calculs de navigation.

### nav-signalk

- Client HTTP.
- Client WebSocket.
- Abonnements.
- Reconnexion.
- Décodage des messages.
- Authentification.

### nav-nmea0183

- Série.
- TCP/UDP.
- Parsing.
- Checksums.
- Conversion vers le modèle interne.

### nav-ais

- Décodage AIS.
- Suivi des cibles.
- Historique.
- CPA/TCPA en V2.

### nav-charts

- Catalogue MBTiles.
- Lecture SQLite.
- Métadonnées.
- Serveur de tuiles.
- Cache.

### nav-routing

- Waypoints.
- Routes.
- Traces.
- GPX.
- Navigation active.

### nav-alarms

- Règles d'alarme.
- Machine à états.
- Historique.
- Notifications.

### nav-storage

- SQLite.
- Migrations.
- Repositories.
- Transactions.

### nav-api

- Axum.
- REST.
- WebSocket.
- Authentification.
- Contrôle d'accès.

## 5. Communication

Desktop local :
- Commandes Tauri.
- Événements ou canaux Tauri.

Mode distant :
- API HTTP.
- WebSocket.

Les structures échangées sont typées et versionnées.

## 6. Acquisition

Les connecteurs transmettent les données à
un bus interne normalisé.

Chaque mesure contient :
- Valeur.
- Source.
- Horodatage de mesure.
- Horodatage de réception.
- Qualité.

L'arbitrage sélectionne la meilleure source
selon priorité, fraîcheur et qualité.

## 7. Concurrence

Utiliser Tokio pour :
- Connexions réseau.
- Lecture série.
- Signal K.
- Traitement AIS.
- Persistance.
- Alarmes.

Ne jamais bloquer le runtime asynchrone
avec des opérations longues.

## 8. Cartographie

MapLibre GL JS assure le rendu.

Rust assure :
- Lecture MBTiles.
- Métadonnées.
- Validation.
- Serveur de tuiles.

Les tuiles sont servies localement.

## 9. Séparation des responsabilités

Le frontend ne doit jamais :
- Décoder directement les flux NMEA.
- Calculer les alarmes critiques.
- Écrire directement dans SQLite.
- Gérer les connexions matérielles.

Le backend ne doit pas dépendre :
- Du DOM.
- De Svelte.
- De la WebView.
- De Tauri pour ses fonctions métier.

## 10. Observabilité

Utiliser tracing avec :
- Logs structurés.
- Niveau configurable.
- Rotation.
- Identifiant de source.
- Événements de reconnexion.
- Erreurs de décodage.
- Déclenchements d'alarmes.
