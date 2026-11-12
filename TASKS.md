
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

- [x] Installer MapLibre GL JS.
- [x] Créer ChartView.svelte.
- [x] Créer nav-charts.
- [x] Lire les métadonnées MBTiles.
- [x] Servir les tuiles raster.
- [x] Charger plusieurs cartes.
- [x] Ajouter zoom et rotation.
- [x] Ajouter le mode Nord en haut.
- [x] Ajouter le mode sombre.
- [x] Afficher une position simulée.

Critères :
- [x] Carte affichée sans Internet.
- [x] Zoom fluide.
- [x] Position simulée visible.

État (2026-11-12) : phase 1 terminée.
- Rust : fmt, clippy (`-D warnings`), 74 tests (dont 8 tests
  d'intégration MBTiles : fichiers corrompus, vides, vectoriels,
  sans tables, cartes multiples, lecture seule).
- Frontend : lint, svelte-check, 70 tests Vitest, build.
- Recette visuelle sous Windows 11 avec deux cartes de test
  générées (`make_test_chart`, voir `fixtures/README.md`) et une
  carte raster réelle de Polynésie (~1 Go, chargée en 0,15 s).
- Tuiles servies par le protocole local `nautik-tiles` ; CSP
  limitée aux sources locales.
- Position simulée fixe dans la passe de Tiputa (Rangiroa) ;
  déplacement simulé après nav-geodesy.
- Niveau de zoom affiché sur la carte.
- Seuil « position obsolète » (5 s) provisoirement côté
  interface ; à porter dans le moteur en phase 3.

## Phase 2 — Navigation

- [x] Créer nav-geodesy.
- [x] Calculer distances et relèvements.
- [x] Créer le modèle Waypoint.
- [x] Créer le modèle Route.
- [x] Créer le modèle Track.
- [x] Ajouter SQLite.
- [x] Créer les migrations.
- [x] Ajouter import/export GPX.
- [x] Ajouter le suivi de route.

Critères :
- [x] Route créée graphiquement.
- [x] Route persistée.
- [x] Trace enregistrée.
- [x] GPX importé et exporté.

État (2026-11-12) : phase 2 terminée (recette visuelle validée sous Windows 11).
- nav-geodesy : géodésiques de Karney (`geographiclib-rs`) :
  distance, relèvements, point de destination, écart de route ;
  conversions d'unités (NM, nœuds).
- nav-routing : modèles Waypoint/Route/Track validés, édition de
  route, navigation active (DTW, BTW, XTE, distance restante, ETA,
  passage de waypoint à 0,1 NM), enregistreur de trace (≥ 5 s et
  ≥ 10 m), simulateur suivant la route active (8 nœuds), GPX 1.1
  (import strict : coordonnée invalide = fichier refusé, traces sans
  horodatage écartées, aucune entité externe résolue).
- nav-storage : SQLx, migration initiale (schéma DATA_MODEL.md),
  clés étrangères, WAL, transactions (import GPX atomique),
  enregistrements interrompus clôturés au démarrage sans inventer
  d'heure.
- Desktop : base `nautik.db` dans le répertoire local de
  l'application ; boucle moteur indépendante de Tauri ; dialogues
  GPX ouverts côté Rust (aucun chemin transmis par le frontend) ;
  traces simulées préfixées « SIMULATION — ».
- Frontend : tracé de route au clic, routes et traces sur la
  carte, panneaux routes / traces / navigation.
- Rust : fmt, clippy (`-D warnings`), 170 tests. Frontend : lint,
  svelte-check, 98 tests Vitest, build.

## Phase 3 — Signal K

- [x] Créer nav-signalk.
- [x] Connexion HTTP.
- [x] Connexion WebSocket.
- [x] Authentification.
- [x] Abonnement aux données.
- [x] Reconnexion automatique.
- [x] Normalisation des unités.
- [x] Affichage des instruments.
- [x] Détection des données obsolètes.

Critères :
- [x] Position reçue de Signal K.
- [x] Instruments actualisés.
- [x] Reconnexion automatique (tests d'intégration sur serveur
  simulé : coupure, refus d'accès, inactivité).

État (2026-11-12) : phase 3 terminée (recette sous Windows 11 avec
Signal K Server et ses données NMEA 0183 d'exemple).
- nav-core : grandeurs SI normalisées et bornées (`Quantity`,
  `Measurement`), état du navire (`VesselState`) calculant la
  fraîcheur à l'horloge de réception (position 5 s, fond et vent
  10 s, autres 5 s).
- nav-signalk : découverte HTTP `/signalk` (sans redirection,
  réponse bornée), WebSocket `subscribe=none` puis abonnement
  explicite, jeton `Bearer` jamais envoyé à un autre hôte ni
  journalisé, contexte limité au navire propre, messages bornés,
  ping et délai d'inactivité, reconnexion à délai exponentiel
  (1 s → 30 s), 401/403 signalés comme accès refusé. TLS rustls
  (ring).
- nav-storage : réglages clé/valeur JSON.
- Desktop : modes exclusifs simulateur / Signal K (aucun repli
  silencieux), jeton dans le coffre du système (Gestionnaire
  d'identification Windows), instantané des instruments diffusé
  chaque seconde, événements d'une session remplacée ignorés.
- Frontend : panneaux Instruments et Source, état de connexion
  toujours visible en mode Signal K, alerte si le moteur se tait
  plus de 3 s.
- Rust : fmt, clippy (`-D warnings`), 208 tests. Frontend : lint,
  svelte-check, 132 tests Vitest, build.

## Phase 4 — NMEA

- [x] Créer nav-nmea0183.
- [x] Lecture TCP.
- [x] Lecture UDP.
- [x] Lecture série.
- [x] Validation checksum.
- [x] Parser RMC/GGA/VTG.
- [x] Parser HDT/HDG.
- [x] Parser DPT/DBT.
- [x] Parser MWV.
- [x] Arbitrage des sources.

Critères :
- [x] Lecture de flux enregistrés.
- [x] Gestion des erreurs.
- [x] Position NMEA affichée.

État (2026-11-12) : phase 4 terminée (recette sous Windows 11).
- nav-nmea0183 (lecture seule, aucune phrase émise) :
  - phrases : somme de contrôle obligatoire, longueur bornée
    (100 caractères), bloc d'étiquettes IEC 61162-450 ignoré ;
  - décodage RMC, GGA, GLL, VTG, HDT, HDG, HDM, VHW, DPT, DBT, MWV
    vers WGS84/SI/UTC ; aucune position si l'équipement la déclare
    invalide, estimée, manuelle ou simulée ; angles hors [0, 360]°
    et valeurs hors bornes rejetés ; vent vrai et variation
    magnétique non appliqués ; AIS ignoré (phase 5) ;
  - liaisons TCP, UDP, série (`tokio-serial`, sans libudev) et rejeu
    de fichier au rythme enregistré ; lignes bornées (512 octets),
    débit écrêté (200 phrases/s), silence > 15 s signalé, reconnexion
    à délai exponentiel (1 s → 30 s).
- nav-core : types `Sample` et `ConnectionState` communs aux
  connecteurs ; arbitrage des sources dans `VesselState` (source
  fraîche, puis priorité configurée, puis la plus récente ; source
  retenue conservée tant qu'elle reste fraîche ; 16 sources au plus
  par donnée) ; seule la source de position retenue alimente la
  navigation et la trace ; source `replay` jamais présentée comme
  réelle.
- Desktop : mode NMEA 0183 (TCP, UDP, série, rejeu), priorités des
  sources, rejets comptés et affichés, fichier de rejeu choisi par le
  dialogue du système ; un rejeu est traité comme une simulation.
- Rust : fmt, clippy (`-D warnings`), 268 tests. Frontend : lint,
  svelte-check, 160 tests Vitest, build.

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
