
# NautiK — Spécifications fonctionnelles

## 1. Présentation

NautiK est une application de navigation maritime
multiplateforme, offline-first, avec un moteur Rust.

Elle permet de consulter des cartes marines, suivre une
navigation, gérer des routes et traces, recevoir des
données NMEA/Signal K et surveiller les cibles AIS.

## 2. Plateformes

- Windows 11 x64
- Linux x64
- Linux ARM64 / Raspberry Pi
- macOS ARM64 et x64

## 3. Cartographie

### MAP-001 — Cartes MBTiles

- Import de fichiers MBTiles raster.
- Catalogue de plusieurs cartes.
- Lecture hors ligne.
- Détection des limites géographiques.
- Gestion des niveaux de zoom.
- Activation/désactivation des cartes.
- Priorité des couches.
- Affichage des attributions.

### MAP-002 — Navigation cartographique

- Zoom, déplacement et rotation.
- Nord en haut.
- Cap en haut.
- Route en haut.
- Suivi automatique du bateau.
- Mesure de distance.
- Mesure de relèvement.
- Affichage de l'échelle.

### MAP-003 — Cartes vectorielles

Version 2 :
- MBTiles vectoriels.
- ENC S-57.
- Symbologie maritime S-52.
- Profondeurs et dangers.

## 4. Navigation

### NAV-001 — Position

Afficher :
- Latitude et longitude.
- Position du bateau.
- COG et SOG.
- Cap vrai et magnétique.
- Profondeur.
- Heure UTC.

### NAV-002 — Waypoints

- Création sur la carte.
- Déplacement.
- Modification.
- Suppression.
- Nom, description et icône.
- Import/export GPX.

### NAV-003 — Routes

- Création graphique.
- Ordre des waypoints.
- Distance totale.
- Relèvement.
- ETA estimée.
- Activation.
- Suivi du waypoint actif.
- XTE.
- Inversion.
- Export GPX.

### NAV-004 — Traces

- Enregistrement automatique.
- Fréquence configurable.
- Historique.
- Affichage cartographique.
- Import/export GPX.

## 5. Signal K

### SIG-001

- Connexion HTTP et WebSocket.
- Authentification par jeton.
- Abonnement aux données.
- Reconnexion automatique.
- Surveillance de fraîcheur.
- Identification des sources.

Chemins prioritaires :
- navigation.position
- navigation.speedOverGround
- navigation.courseOverGroundTrue
- navigation.headingTrue
- navigation.headingMagnetic
- navigation.speedThroughWater
- environment.depth.belowTransducer
- environment.wind.speedApparent
- environment.wind.angleApparent

## 6. NMEA

### NMEA-001 — NMEA 0183

Transports :
- Série USB.
- TCP.
- UDP.

Messages :
RMC, GGA, GLL, VTG, HDT, HDG,
DPT, DBT, MWV, VHW, VDM, VDO.

### NMEA-002 — NMEA 2000

MVP :
- Données via Signal K.

Version 2 :
- SocketCAN Linux.
- Décodage des PGN nécessaires.
- Lecture seule par défaut.

## 7. AIS

### AIS-001

- Décodage VDM/VDO.
- MMSI.
- Nom.
- Position.
- COG/SOG.
- Cap.
- Statut de navigation.
- Dernière réception.

### AIS-002

- Affichage cartographique.
- Symboles différenciés.
- Vecteurs de déplacement.
- Historique des positions.
- Cibles perdues.

### AIS-003

Version 2 :
- CPA.
- TCPA.
- Seuils configurables.
- Alarmes anticollision.

## 8. Alarmes

### ALR-001 — Mouillage

- Définition du point d'ancre.
- Rayon configurable.
- Calcul de distance.
- Temporisation.
- Hystérésis.
- Alarme sonore.
- Historique.

### ALR-002 — Autres alarmes

- Perte GPS.
- Perte Signal K.
- Profondeur.
- Écart de route.
- AIS CPA/TCPA.
- Batterie.

## 9. Interface

- Mode jour/nuit.
- Carte plein écran.
- Barre d'outils.
- Panneaux latéraux.
- Instruments configurables.
- Support tactile.
- Adaptation aux résolutions.

## 10. Embarqué

- Service Rust autonome.
- systemd.
- API locale et distante.
- Redémarrage automatique.
- Watchdog.
- Fonctionnement sans écran.
- Raspberry Pi ARM64.

## 11. Contraintes

- Offline-first.
- Stockage local SQLite.
- Aucune dépendance cloud obligatoire.
- Séparation moteur / interface.
- Données horodatées en UTC.
- Coordonnées WGS84.
- Tests automatisés.
