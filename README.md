# NautiK

**La navigation à la voile et au moteur, sur votre ordinateur, même au
large et sans Internet.**

NautiK est une application de navigation maritime libre et gratuite. Elle
affiche vos cartes, la position du bateau, vos instruments, les navires
AIS alentour et veille sur vous (homme à la mer, mouillage, perte de
position), sur un PC portable de bord comme sur un Raspberry Pi.

> ⚠️ **NautiK est une aide à la navigation, pas un ECDIS certifié.** Elle
> ne remplace ni les cartes officielles à jour, ni la veille, ni le sens
> marin du chef de bord.

## Ce que fait NautiK

### Cartes

- **Cartes raster MBTiles** (SHOM, NOAA, cartes converties…), avec un nom
  personnalisable pour chacune.
- **Cartes vectorielles officielles ENC (S-57)**, mises à jour comprises :
  sondes, isobathes, balisage, feux, dangers.
- **Base CM93** pour couvrir les zones sans carte officielle (signalée
  comme non officielle).
- **Îles et amers OpenStreetMap** préparés à l'avance pour l'usage hors
  ligne.
- **Deux cartes côte à côte**, synchronisées ou non, pour comparer deux
  sources ou deux échelles.
- **Recherche instantanée** d'un lieu, d'un point ou d'une coordonnée,
  dès la frappe.
- La carte **suit le bateau** quand vous le souhaitez.

### Navigation

- **Routes** tracées à la souris, avec longueurs et caps des segments ;
  **« Naviguer jusqu'ici »** en un clic.
- Suivi en route : relèvement et distance au prochain point, **écart de
  route (XTE)**, heure d'arrivée estimée.
- **Traces** enregistrées d'un clic et **relecture** de vos
  sorties.
- **Marques** (points d'intérêt), **vecteur de cap** et **laylines**.
- **Gestionnaire de routes, traces et marques** ; import et export
  **GPX** et **KML** (Google Earth).
- **Journal de bord**.

### Instruments et AIS

- **Barre Navigation** configurable : vitesse, cap, profondeur, vent,
  angle de barre… les valeurs de votre choix.
- **Navires AIS** sur la carte, avec vecteur de route, historique et liste
  des cibles.

### Sécurité

- **Homme à la mer** en un geste (bouton ou Ctrl+M) : relèvement,
  distance et temps écoulé affichés en grand.
- **Alarme de mouillage** avec cercle d'évitage.
- **Alarme de perte de position.**
- Les alarmes sonnent jusqu'à l'acquittement et fonctionnent même sans
  écran (service Raspberry Pi).
- NautiK **n'invente jamais une position** : donnée ancienne, liaison
  coupée ou GPS douteux sont toujours affichés clairement.

### Météo

- **Vent prévu** (fichiers GRIB2) : import d'un fichier reçu par courriel,
  satellite ou clé USB, ou téléchargement GFS de la zone affichée, à la
  demande seulement.

## Se raccorder au bateau

NautiK lit les données de bord depuis :

- un **serveur Signal K** (OpenPlotter, etc.) ;
- un multiplexeur ou une passerelle Wi-Fi **NMEA 0183** (TCP ou UDP) ;
- un GPS ou multiplexeur **USB / série** ;
- un **simulateur** intégré, pour découvrir l'application à terre, ou la
  relecture d'un enregistrement.

NautiK n'envoie jamais de commande aux équipements du bord.

## Installation

| Système | Paquet |
| --- | --- |
| Windows 10 / 11 | `NautiK_<version>_x64-setup.exe` (sans droits d'administrateur) ou `.msi` |
| Linux (Debian, Ubuntu) | `.deb`, ou `.AppImage` pour les autres distributions |
| Raspberry Pi (sans écran) | service `nautikd` : voir [apps/daemon/README.md](apps/daemon/README.md) |

Les installateurs sont publiés sur la page des
[versions](https://github.com/laurentpellet/NautiK/releases) du projet.

Au premier lancement, choisissez votre source de données et le dossier de
vos cartes dans **Paramètres** : tout est expliqué dans le
**[guide utilisateur](docs/guide-utilisateur.md)**.

## Vos données restent chez vous

- Aucun compte, aucun service en ligne obligatoire : tout fonctionne hors
  ligne.
- Routes, traces, réglages et historique sont enregistrés sur votre
  ordinateur.
- Seuls les téléchargements que vous demandez (météo) utilisent Internet.

## État du projet

NautiK est en développement actif (version 0.2). Le calcul de
rapprochement AIS (CPA/TCPA) et l'interface web à distance sont prévus ;
voir le [plan de développement](TASKS.md).

## Pour les développeurs

Application de bureau Tauri 2 (Svelte 5, MapLibre) au-dessus d'un moteur
Rust indépendant de l'interface, aussi utilisable comme service `nautikd`
(API REST et WebSocket). Documents de référence :
[SPEC.md](SPEC.md), [ARCHITECTURE.md](ARCHITECTURE.md),
[DATA_MODEL.md](DATA_MODEL.md), [SECURITY.md](SECURITY.md),
[TEST_PLAN.md](TEST_PLAN.md) et [AGENTS.md](AGENTS.md) (consignes pour
les agents de développement).

## Licence

À définir après audit des dépendances.
