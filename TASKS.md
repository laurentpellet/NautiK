
# NautiK — Plan de développement

## Règles

- Développer par phases.
- Chaque phase doit compiler.
- Chaque fonctionnalité doit être testée.
- Ne pas commencer la phase suivante si
  les critères de recette échouent.
- Maintenir ce fichier à jour.

## Phase 0 — Initialisation

- [x] Créer le workspace Cargo.
- [x] Créer l'application Tauri 2.
- [x] Installer Svelte 5 et TypeScript.
- [x] Configurer Vite.
- [x] Configurer ESLint et Prettier.
- [x] Configurer rustfmt et Clippy.
- [x] Ajouter tracing.
- [x] Configurer GitHub Actions.
- [x] Créer la structure des crates.

Critères :
- [x] Application desktop démarrable.
- [x] Compilation Rust réussie.
- [x] Tests exécutables.

État (2026-11-12) : phase 0 terminée.
- Rust : `cargo fmt --check`, `cargo clippy` (0 avertissement)
  et `cargo test` (26 tests, `-D warnings`) réussis.
- Frontend : lint, svelte-check, 16 tests Vitest et build réussis.
- `npm run tauri dev` : application lancée sous Windows 11.
- Prérequis Windows : charge « Développement Desktop en C++ »
  de Visual Studio (MSVC + Windows SDK).

## Phase 1 — Cartographie

- [ ] Installer MapLibre GL JS.
- [ ] Créer ChartView.svelte.
- [ ] Créer nav-charts.
- [ ] Lire les métadonnées MBTiles.
- [ ] Servir les tuiles raster.
- [ ] Charger plusieurs cartes.
- [ ] Ajouter zoom et rotation.
- [ ] Ajouter le mode Nord en haut.
- [ ] Ajouter le mode sombre.
- [ ] Afficher une position simulée.

Critères :
- Carte affichée sans Internet.
- Zoom fluide.
- Position simulée visible.

## Phase 2 — Navigation

- [ ] Créer nav-geodesy.
- [ ] Calculer distances et relèvements.
- [ ] Créer le modèle Waypoint.
- [ ] Créer le modèle Route.
- [ ] Créer le modèle Track.
- [ ] Ajouter SQLite.
- [ ] Créer les migrations.
- [ ] Ajouter import/export GPX.
- [ ] Ajouter le suivi de route.

Critères :
- Route créée graphiquement.
- Route persistée.
- Trace enregistrée.
- GPX importé et exporté.

## Phase 3 — Signal K

- [ ] Créer nav-signalk.
- [ ] Connexion HTTP.
- [ ] Connexion WebSocket.
- [ ] Authentification.
- [ ] Abonnement aux données.
- [ ] Reconnexion automatique.
- [ ] Normalisation des unités.
- [ ] Affichage des instruments.
- [ ] Détection des données obsolètes.

Critères :
- Position reçue de Signal K.
- Instruments actualisés.
- Reconnexion automatique.

## Phase 4 — NMEA

- [ ] Créer nav-nmea0183.
- [ ] Lecture TCP.
- [ ] Lecture UDP.
- [ ] Lecture série.
- [ ] Validation checksum.
- [ ] Parser RMC/GGA/VTG.
- [ ] Parser HDT/HDG.
- [ ] Parser DPT/DBT.
- [ ] Parser MWV.
- [ ] Arbitrage des sources.

Critères :
- Lecture de flux enregistrés.
- Gestion des erreurs.
- Position NMEA affichée.

## Phase 5 — AIS

- [ ] Créer nav-ais.
- [ ] Décoder VDM/VDO.
- [ ] Gérer les fragments AIS.
- [ ] Créer le modèle AisTarget.
- [ ] Afficher les cibles.
- [ ] Afficher COG/SOG.
- [ ] Gérer les cibles obsolètes.
- [ ] Ajouter historique.

Critères :
- Flux AIS simulé décodé.
- Cibles visibles sur la carte.
- Aucune panne sur message invalide.

## Phase 6 — Alarmes

- [ ] Créer nav-alarms.
- [ ] Définir machine à états.
- [ ] Ajouter alarme mouillage.
- [ ] Ajouter perte GPS.
- [ ] Ajouter hystérésis.
- [ ] Ajouter temporisation.
- [ ] Ajouter alarme sonore.
- [ ] Historiser les événements.

Critères :
- Alarme mouillage fonctionnelle.
- Historique conservé.
- Alarmes indépendantes de Svelte.

## Phase 7 — Service embarqué

- [ ] Développer le service nautikd (apps/daemon).
- [ ] Créer API Axum.
- [ ] Ajouter WebSocket.
- [ ] Authentifier les clients.
- [ ] Ajouter service systemd.
- [ ] Compiler Linux ARM64.
- [ ] Tester Raspberry Pi.
- [ ] Ajouter watchdog.
- [ ] Documenter installation.

Critères :
- Service sans interface.
- Reconnexion après panne.
- Client distant fonctionnel.

## Phase 8 — Stabilisation

- [ ] Tests d'intégration.
- [ ] Tests de charge AIS.
- [ ] Tests coupure réseau.
- [ ] Tests coupure alimentation.
- [ ] Mesures CPU/RAM.
- [ ] Packaging Windows.
- [ ] Packaging Linux.
- [ ] Documentation utilisateur.

## Phase 9 — Évolutions

- [ ] Cartes ENC S-57.
- [ ] CPA/TCPA.
- [ ] NMEA 2000 SocketCAN.
- [ ] Météo GRIB.
- [ ] Marées et courants.
- [ ] Routage météo.
- [ ] Intégration pilote automatique.
