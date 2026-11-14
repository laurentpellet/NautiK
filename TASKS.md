
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

- [x] Créer nav-ais.
- [x] Décoder VDM/VDO.
- [x] Gérer les fragments AIS.
- [x] Créer le modèle AisTarget.
- [x] Afficher les cibles.
- [x] Afficher COG/SOG.
- [x] Gérer les cibles obsolètes.
- [x] Ajouter historique.

Critères :
- [x] Flux AIS simulé décodé.
- [x] Cibles visibles sur la carte.
- [x] Aucune panne sur message invalide (tests de messages malformés).

État (2026-11-12) : phase 5 terminée (recette sous Windows 11 en mode simulateur).
- nav-ais (lecture seule) :
  - fragments VDM/VDO validés (nombre, numéro, identifiant, canal,
    alphabet, bits de remplissage) ; réassemblage dans l'ordre,
    16 messages en cours au plus, 5 s au plus, 256 caractères au
    plus ; fragment orphelin, hors ordre ou incomplet rejeté ;
  - messages 1, 2, 3, 5, 18, 19, 24 (parties A et B) ; autres types
    ignorés ; MMSI 1 à 999 999 999 ; valeurs « non disponible » et
    réservées (position 181°/91° ou hors bornes, SOG 1023, COG ≥ 3600,
    cap ≥ 360, statut 15, type 0) → absentes, jamais inventées ;
  - suivi des cibles : fusion par MMSI, fraîcheur selon l'intervalle
    nominal ITU-R M.1371 (classe, vitesse, mouillage) : obsolète
    au-delà de 3 intervalles, perdue au-delà de 6 ou sans position,
    retirée après 30 min de silence ; messages VDO (navire propre)
    ignorés ; 2 000 cibles au plus ; historique en mémoire (une
    position par 30 s, 20 min) ; vecteur de déplacement à 6 min
    calculé par le moteur (cible active seulement) ;
  - flux simulé : 3 navires fictifs (MMSI 97000000x, noms « SIM … »)
    émis en phrases `!AIVDM` complètes, décodées par le même chemin
    que les flux réels.
- nav-nmea0183 : phrases `!AIVDM`/`!AIVDO` transmises comme fragments
  (somme de contrôle obligatoire, débit écrêté) ; erreurs AIS comptées
  parmi les rejets de la liaison.
- Desktop : cibles oubliées à chaque changement de source ; événement
  `ais-targets` chaque seconde ; cibles simulées ou rejouées marquées.
- Frontend : symboles SDF générés localement (triangle orienté par le
  cap, sinon la route ; cercle sans direction ; classe A / B
  différenciées ; obsolète estompée, perdue grisée), vecteurs COG/SOG,
  historique en pointillés, panneau AIS (SOG, COG, état, sélection
  au clic sur la carte, bandeau SIMULATION).
- Reportés : persistance de la table `ais_targets` ; cibles AIS reçues
  par Signal K (contexte limité au navire propre) ; CPA/TCPA (V2) ;
  mesures de charge (phase 8).
- Rust : fmt, clippy (`-D warnings`), 310 tests. Frontend : lint,
  svelte-check, 186 tests Vitest, build.
- Carte : zoom 10 au moins au démarrage, centrée sur la première
  position reçue (sauf si la carte a déjà été déplacée).

### Interface (hors phase, avant la phase 6)

- [x] Onglet Instruments : tableau de bord sombre (SOG, fond, surface,
      route, cap, vent apparent), cadran de vent (secteurs bâbord /
      tribord), tendance du vent apparent sur 1 h sans interpolation
      des trous, navigation active et position. Données obsolètes
      estompées et signalées ; aucune donnée inventée.
- [x] Plein écran complet (bouton, F11, Échap) ; alertes de position et
      de liaison toujours visibles.
- [x] Onglet Paramètres : unités (vitesse, distance, profondeur),
      vitesse du bateau simulé (persistée, appliquée par le moteur),
      sources.
- [x] Refonte visuelle : barre d'en-tête (onglets, état position /
      liaison / AIS / moteur, compteur d'alertes, heure UTC, thème,
      plein écran), thème jour bleu marine (nuit inchangé), menu des
      calques (un calque masqué reste signalé), carte « Navigation
      temps réel » avec compas, cartes Systèmes et Données historiques
      (mémoire seule, depuis le lancement, 15 min / 1 h / 6 h).
- [x] Simulateur complet : vent réel, températures eau / air,
      pression, moteur (régime, température, pression d'huile),
      réservoirs (gasoil, eau douce), batterie (charge, tension,
      courant), solaire. Grandeurs SI ajoutées à `nav_core::Quantity` ;
      vent apparent calculé depuis le vent réel et le déplacement ;
      moteur arrêté au mouillage ; mode simulateur seulement (rien
      d'inventé en mode Signal K). Cartes Moteur, Énergie, Réservoirs,
      Environnement dans l'onglet Instruments.

### Météo GRIB (phase 9, avancée — recette OK le 2026-11-12)

- [x] Crate nav-weather (sans dépendance C) : décodeur GRIB2 (grille
      lat/lon 3.0, produits 4.0/4.1, compactage 5.0, 5.2, 5.3 ordres 1
      et 2, bitmap) ; champs non pris en charge ignorés et signalés ;
      fichier 200 Mo et 4 M points au plus ; vent U/V à 10 m.
- [x] Téléchargement GFS 0,25° optionnel (filtre NOMADS, zone affichée
      réduite à 120° × 90°, 24 à 120 h par pas de 3 h, run précédent si
      absent, réponse vérifiée « GRIB », 20 Mo par échéance) ; jamais
      automatique, Internet non requis pour l'usage.
- [x] Bibliothèque hors ligne (dossier `weather`, écriture atomique,
      identifiants validés), import par le dialogue du système,
      suppression confirmée.
- [x] Onglet Météo ; flèches de vent colorées par la force sur la carte
      (zone affichée, 1 500 points au plus), barre d'échéances, run et
      prévision ancienne / échéance passée signalés ; avertissement :
      aide à la décision, pas un bulletin officiel.
- Reportés : rafales, pression, vagues, courants, GRIB1, grilles non
  régulières, interpolation temporelle, routage météo.
- Rust : fmt, clippy (`-D warnings`), 341 tests (+ 1 test GFS réel
  ignoré par défaut, vérifié le 2026-11-12). Frontend : lint,
  svelte-check, 253 tests Vitest, build.

## Phase 6 — Alarmes

- [x] Créer nav-alarms.
- [x] Définir machine à états.
- [x] Ajouter alarme mouillage.
- [x] Ajouter perte GPS.
- [x] Ajouter hystérésis.
- [x] Ajouter temporisation.
- [x] Ajouter alarme sonore.
- [x] Historiser les événements.

Critères :
- [x] Alarme mouillage fonctionnelle (recette sous Windows 11 en mode
      simulateur).
- [x] Historique conservé (table `alarm_events`, transaction).
- [x] Alarmes indépendantes de Svelte (évaluées par le moteur à chaque
      pas de la boucle, sans interface).

État (2026-11-13) : phase 6 terminée (recette sous Windows 11 en mode simulateur).
- nav-alarms (sans E/S) :
  - machine à états Normal → En attente → Active → Acquittée → Normal ;
    temporisation avant déclenchement, retour stable de 5 s avant
    résolution ; une donnée absente ou obsolète ne résout ni ne
    déclenche jamais une alarme ;
  - mouillage (ALR-001) : ancre posée à la position courante, qui doit
    être fraîche (aucune position supposée) ; rayon 10 à 2 000 m,
    hystérésis 10 % du rayon (5 m au moins), temporisation 0 à 300 s ;
  - perte de position (ALR-002) : position absente ou obsolète, après
    une première position reçue ou dès que le mouillage est surveillé ;
    une alarme active le reste après un changement de source ;
  - désactiver la surveillance ou lever l'ancre est explicite et
    historisé ; une alarme acquittée reste affichée.
- nav-storage : configuration (`alarms`, reprise au redémarrage, valeurs
  illisibles signalées) et historique (`alarm_events`) ; pas de
  migration (tables du schéma initial).
- Desktop : événement `alarms` chaque seconde, déclenchements
  journalisés (`tracing`), commandes `get_alarms`, `drop_anchor`,
  `raise_anchor`, `acknowledge_alarm`, `set_alarm_enabled`,
  `list_alarm_events`.
- Frontend : bandeau d'alarme dans tous les onglets et en plein écran
  (acquittement), pastille « Alarmes » dans l'en-tête, carte Mouillage
  (distance, levée confirmée), cercle d'évitage et limite de retour
  sur la carte, son généré localement (Web Audio) tant que l'alarme
  n'est pas acquittée, Paramètres → Alarmes (désactivation confirmée,
  historique).
- Reportés : alarmes profondeur, écart de route, batterie, perte
  Signal K distincte de la perte de position, CPA/TCPA (V2) ; ancre
  posée à une position choisie sur la carte.
- Rust : fmt, clippy (`-D warnings`), 373 tests. Frontend : lint,
  svelte-check, 274 tests Vitest, build.

## Phase 7 — Service embarqué

- [x] Développer le service nautikd (apps/daemon).
- [x] Créer API Axum.
- [x] Ajouter WebSocket.
- [x] Authentifier les clients.
- [x] Ajouter service systemd.
- [x] Compiler Linux ARM64 (job CI « Service ARM64 », exécuteur
      `ubuntu-24.04-arm`, artefact `nautikd-linux-arm64`).
- [ ] Tester Raspberry Pi (pas de Pi disponible pour l'instant ; outils
      prêts : paquet `.deb` arm64, `scripts/measure-nautikd.sh`).
- [x] Ajouter watchdog.
- [x] Documenter installation (`apps/daemon/README.md`).

Critères :
- [x] Service sans interface (test de bout en bout : démarrage, boucle,
      API, arrêt propre).
- [ ] Reconnexion après panne (redémarrage systemd, reconnexion des
      liaisons et reprise des réglages en place ; à vérifier sur le Pi).
- [ ] Client distant fonctionnel (API REST/WebSocket testée ; interface
      web servie par nautikd à faire).

État (2026-11-13) : phase 7 développée, recette Raspberry Pi à faire.
- nav-engine (nouveau crate, sans Tauri) : moteur, boucle périodique,
  connecteurs, opérations (alarmes, routes, traces, sources, simulateur)
  extraits de l'application desktop, qui n'est plus qu'une couche de
  commandes Tauri. Les changements sont diffusés sur un canal
  (`EngineEvent`) relayé en événements Tauri (noms inchangés) ou en
  WebSocket.
- nav-api (Axum) : REST `/api/v1/…`, WebSocket `/api/v1/ws` (état initial
  puis flux), `/health` public. Jeton d'accès (en-tête Bearer ou premier
  message WebSocket, comparaison en temps constant, jamais journalisé),
  corps limités à 256 Kio, 16 sessions WebSocket au plus, champs inconnus
  refusés, rejeu de fichier refusé à distance.
- nautikd : options `--data-dir`, `--listen` (127.0.0.1:8375 par défaut,
  ouverture au réseau explicite), `--rotate-token` ; jeton et coffre des
  secrets dans des fichiers `0600` ; chien de garde systemd réarmé
  seulement si la boucle progresse ; arrêt propre sur SIGTERM (trace
  clôturée, clients prévenus). Unité systemd durcie avec limites de
  ressources.
- Reporté : alarme sonore sur le service (buzzer), interface web servie
  par nautikd, cartes et météo via l'API, surveillance du disque.
- Rust : fmt, clippy (`-D warnings`), 403 tests. Frontend : lint,
  svelte-check, 274 tests Vitest, build.

## Phase 8 — Stabilisation

- [x] Tests d'intégration (`crates/nav-engine/tests/pipeline.rs` : liaison
      TCP réelle → connecteur → arbitrage → boucle → événements).
- [x] Tests de charge AIS (`crates/nav-ais/tests/load.rs` : 100, 1 000 et
      10 000 cibles).
- [x] Tests coupure réseau (perte signalée, position obsolète conservée
      sans extrapolation, alarme, reconnexion automatique ; source
      injoignable sans position inventée).
- [x] Tests coupure alimentation (`apps/daemon/tests/power_cut.rs` :
      `nautikd` tué pendant un enregistrement, ancre mouillée).
- [x] Mesures CPU/RAM (`scripts/measure-nautikd.ps1` / `.sh`, multiplexeur
      simulé `cargo run -p nav-engine --example nmea_feed`). Raspberry Pi
      à mesurer.
- [x] Packaging Windows (MSI, NSIS ; workflow « Paquets » ; installateur NSIS
      construit localement : `NautiK_0.1.0_x64-setup.exe`, 5,1 Mio).
- [x] Packaging Linux (application : deb, AppImage ; service : `.deb`
      amd64 et arm64 par `apps/daemon/packaging/build-deb.sh`).
- [x] Documentation utilisateur (`docs/guide-utilisateur.md`).

Critères (TEST_PLAN.md §7) :
- [x] Le code compile ; les tests passent.
- [x] Résilience réseau et alimentation vérifiées par des tests
      automatisés.
- [ ] Paquets Linux et `.deb` du service construits par la CI (à vérifier
      au premier lancement du workflow « Paquets »).
- [ ] Mesures sur Raspberry Pi.

État (2026-11-13) : phase 8 développée ; Raspberry Pi non disponible.
- Mesures (Windows 11, 12 cœurs, `nautikd` release, flux TCP simulé,
  90 s) :

  | Cibles AIS | Phrases/s | Cibles suivies | CPU moyen / pic (1 cœur) | RAM (working set) |
  | --- | --- | --- | --- | --- |
  | 100 | ~10 | 100 | 0,0 % / 0,0 % | 10,9 Mio |
  | 1 000 | ~100 | 1 000 | 0,4 % / 3,1 % | 11,4 à 11,9 Mio |
  | 2 000 | ~200 | 2 000 | 0,6 % / 2,3 % | 11,4 à 13,1 Mio |

  Démarrage du service : ~0,6 s. Décodage et suivi AIS (release) :
  ~1 µs par phrase ; instantané de 1 000 cibles : 1,6 ms ; 10 000 cibles
  plafonnées à 2 000 sans croissance de la mémoire.
- Constat : à 2 000 cibles le flux atteint l'écrêtage du connecteur
  (200 phrases/s), signalé comme rejet ; toutes les cibles restent
  suivies. Un récepteur AIS réel plafonne vers 75 messages/s (capacité
  des deux canaux) : limite conservée.
- Flotte AIS simulée (`AisSimulator::fleet`, MMSI 971…, au plus 20 000)
  aux émissions étalées comme le partage du temps TDMA.
- Base vérifiée au démarrage (`PRAGMA quick_check`) par l'application et
  le service : anomalie journalisée sans bloquer la navigation.
- Coupure d'alimentation : base intègre (WAL), points conservés, trace
  clôturée à l'heure de son dernier point, mouillage repris.
- Rust : fmt, clippy (`-D warnings`), 414 tests. Frontend : lint,
  svelte-check, 274 tests Vitest, build.

## Phase 9 — Évolutions

- [ ] Cartes ENC S-57 (en cours).
  - [x] Dossier ENC : choix dans Paramètres → Cartes (dialogue du
    système, mémorisé dans la base, réglage `enc_dir`), dossier disparu
    signalé ; inventaire `nav_charts::enc` : cellules `.000`, mises à jour
    `.001`…, catalogues `CATALOG.031`, parcours borné (profondeur 8,
    50 000 entrées, liens symboliques ignorés).
  - [x] Décodage ISO 8211 / S-57 des cellules de base (`nav_charts::s57`) :
    - lecteur ISO 8211 borné (guide, répertoire, DDR, contrôles de
      format) ; implémentation binaire ENC seule (ASCII refusée) ;
    - DSID/DSSI/DSPM : fichier de mise à jour (EXPP 2) refusé, WGS 84
      (HDAT 2) et mètres (DUNI 1) imposés, COMF/SOMF non nuls ; nombres
      d'enregistrements DSSI vérifiés (cellule tronquée entre deux
      enregistrements détectée) ;
    - vecteurs (VRID, VRPT, SG2D, SG3D) et objets (FRID, ATTF, NATF en
      Latin-1 ou UCS-2, FSPT) ; géométrie WGS 84 `[lon, lat]` : points,
      sondes (m), lignes (arcs enchaînés, arcs masqués non tracés),
      surfaces (contours fermés, extérieur d'abord, trous) ;
    - objet incohérent (référence manquante, contour non fermé,
      coordonnée hors limites, instruction de mise à jour) écarté et
      compté, jamais de position inventée ; 20 problèmes détaillés au
      plus ; fichier de 64 Mio au plus, lu hors du runtime ;
    - tests : cellule fabriquée par un encodeur ISO 8211 de test, chaque
      troncature rejetée, octets corrompus sans panique (≈ 4 000
      variantes), formats invalides.
    - Application : bouton « Lire » par cellule (commande
      `inspect_enc_cell`, chemin relatif vérifié par
      `resolve_cell_path` : ni `..`, ni chemin absolu, ni sortie du
      dossier par lien) ; résumé (édition, date, usage, échelle, classes,
      sondes, emprise), objets écartés signalés.
    - Vérifié sur deux cellules NOAA réelles (US5MA1AM, port, 1 538
      objets, 1 177 sondes ; US4MA23M, approche, 1 748 objets) : aucun
      objet écarté, emprise et attributs cohérents (bouées nommées,
      DRVAL1/DRVAL2, VALSOU), ≈ 70 ms par cellule ; `.001` et
      `CATALOG.031` refusés comme cellules de base. Outil :
      `cargo run -p nav-charts --example enc_summary -- CELLULE.000`
      (`NAUTIK_ENC_CLASS=n` pour lister les attributs d'une classe).
  - [x] Application des mises à jour `.001`… (`nav_charts::s57::update`) :
    - fichiers `NOM.001` à `NOM.999` trouvés à côté de la cellule (casse
      ignorée), appliqués dans l'ordre par `decode_cell_file` ;
    - EXPP 2 et même édition exigés ; numéros UPDN consécutifs (mise à
      jour manquante signalée), mises à jour déjà intégrées à une
      réédition ignorées ;
    - instructions RUIN (insérer, supprimer, modifier) avec contrôle des
      versions RVER ; listes modifiées par VRPC, SGCC (2D et 3D), FSPC ;
      attributs remplacés, ajoutés ou supprimés (`DEL`) ;
    - chaque mise à jour est appliquée sur une copie, en entier ou pas
      du tout ; la première refusée arrête la séquence : cellule
      signalée « pas à jour » avec le nombre de mises à jour non
      appliquées et la raison ;
    - annulation par le producteur (mise à jour d'édition 0) signalée
      comme alerte : « ne plus l'utiliser pour la navigation » ;
    - tests : insertions, suppressions, déplacement de nœud, retrait de
      trou, versions et cibles incohérentes, séquence trouée, réédition,
      annulation, chaque troncature d'une mise à jour (jamais appliquée
      en partie), octets corrompus, recherche des fichiers ;
    - vérifié sur NOAA : US5MA1AM, 7 mises à jour appliquées sans objet
      écarté (1 538 → 1 552 objets), puis annulation du 30/12/2022 ;
      US4MA23M annulée le 09/10/2024.
    - Application : résumé avec mises à jour appliquées et date de la
      dernière, cellule pas à jour ou annulée signalée.
  - [x] Affichage sur la carte, symbologie S-52 simplifiée (profondeurs,
    dangers) :
    - moteur (`nav_charts::s57::display`) : catégories d'affichage
      (terre, zones de profondeur, zones non hydrographiées, isobathes,
      trait de côte, sondes, dangers, feux, bouées, balises) et entités
      GeoJSON WGS 84, une par sonde, étiquette de sonde calculée ; rien
      pour une cellule annulée ;
    - `nav_charts::EncLibrary` : index des cellules (emprise, usage,
      mises à jour) gardé tant que les fichiers ne changent pas, entités
      des cellules en cache (32 au plus) ; commandes
      `get_enc_map_index` et `get_enc_cell_features` (chemin relatif
      vérifié, cellule annulée refusée) ;
    - interface : cellules de la vue choisies (16 au plus, les plus
      détaillées par-dessus), couches MapLibre jour et nuit au-dessus
      des cartes raster et sous routes, traces et AIS ; zones de
      profondeur colorées selon les isobathes 2, 10 et 30 m ; sondes
      en images (pas de polices hors ligne), en gras sous 10 m, à
      partir du zoom 12 ; dangers marqués sous 10 m ou de profondeur
      inconnue ;
    - calque « Cartes ENC (S-57) » dans le menu Calques ; avertissement
      permanent sur la carte : cellule pas à jour, annulée, illisible,
      index incomplet, cellules non affichées faute de place ;
    - tests : catégories et étiquettes, cache et signatures, parseurs
      (données malformées, positions hors limites, catégorie inconnue),
      choix des cellules (antiméridien), contrôleur et avertissements ;
    - vérifié sur NOAA US5MA1AM (7 mises à jour) : 1 999 entités, jour
      et nuit.
  - [x] Base C-MAP CM93 v2 (cartes non officielles), lecture et
    affichage (`nav_charts::cm93`) :
    - table de substitution des octets reprise d'OpenCPN (GPL-2.0 ou
      ultérieure, provenance notée dans `cm93/table.rs`), lecteur
      propre : prologue et longueurs contrôlés, en-tête, vecteurs,
      points 2D/3D, objets (points, lignes, surfaces, sondes) et
      attributs typés par `CM93ATTR.DIC` ; classes par `CM93OBJ.DIC` ;
    - coordonnées Mercator sphérique (R = 6 378 388 m) → WGS 84 ;
      décalage WGS 84 déclaré non appliqué (comme OpenCPN) mais signalé ;
      objet incohérent écarté et compté, cellule incohérente refusée ;
    - inventaire `LLLLGGGG/<échelle>/fichiers` (8 échelles Z à G, 100 000
      cellules au plus), choix des cellules de la vue de la moins à la
      plus détaillée selon l'étendue (16 au plus, cellules omises
      comptées) ; contours des surfaces orientés par imbrication (sens
      du fichier non fiable) ; ITDARE dessinée comme zone découvrante ;
    - base détectée dans le dossier ENC (le dossier lui-même, un
      sous-dossier ou un sous-sous-dossier) sans la parcourir comme des
      ENC ; commandes `get_cm93_view` et `get_cm93_cell_features`
      (chemin vérifié contre l'index, décodage hors du runtime, cache de
      48 cellules) ;
    - interface : CM93 dessinée sous les ENC officielles avec la même
      symbologie ; avertissements permanents : carte non officielle et
      date des données, décalage déclaré non corrigé, cellules omises
      (« zoomez ») ou illisibles, base illisible ;
    - choix séparé des ENC et de la CM93 : calque « Cartes CM93 » du
      menu Calques (proposé si une base est trouvée) et choix propre au
      volet de droite des cartes côte à côte (mémorisé), pour comparer
      les deux ; seules les cartes voulues dans un volet sont chargées
      et signalées ;
    - tests : dictionnaires, cellule fabriquée par un encodeur de test,
      chaque troncature, octets corrompus, attributs, coordonnées,
      noms et échelles, antiméridien, parcours, vues, parseurs et
      contrôleur ;
    - vérifié sur la base « CM93 2018 » (cartes 2014) : 22 205 cellules
      indexées en 110 ms, 19,6 millions d'objets décodés (1 écarté),
      3 cellules incohérentes refusées ; Papeete (`02170631.F`, 2 417
      objets) affichée jour et nuit. Outil :
      `cargo run --release -p nav-charts --example cm93_summary -- BASE`.
- [ ] CPA/TCPA.
- [ ] NMEA 2000 SocketCAN.
- [x] Météo GRIB : vent à 10 m (voir plus haut ; autres champs reportés).
- [ ] Marées et courants.
- [ ] Routage météo.
- [ ] Intégration pilote automatique.
