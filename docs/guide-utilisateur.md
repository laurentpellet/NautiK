# NautiK — Guide utilisateur

NautiK est une application de navigation maritime qui fonctionne sans
Internet : cartes hors ligne, position GPS, instruments, AIS, routes,
traces, alarmes de mouillage et de perte de position, prévisions de vent.

> **Avertissement.** NautiK est une aide à la navigation. Ce n'est **pas
> un ECDIS certifié** et il ne remplace ni les cartes officielles, ni la
> veille visuelle et radar, ni les moyens de navigation indépendants
> exigés à bord. Les essais en mer doivent se faire progressivement (à
> quai, au mouillage, puis en navigation côtière), avec un autre moyen de
> positionnement.

## 1. Installation

### Windows

Installez le paquet `NautiK_<version>_x64-setup.exe` (installation pour
l'utilisateur courant, sans droits d'administrateur) ou
`NautiK_<version>_x64_fr-FR.msi` (déploiement géré). Les installateurs
sont produits par le workflow « Paquets » du dépôt.

### Linux (PC)

- Debian, Ubuntu : `sudo apt install ./NautiK_<version>_amd64.deb` ;
- autres distributions : rendez exécutable le fichier `.AppImage` puis
  lancez-le.

### Raspberry Pi (service sans écran)

Le service `nautikd` fait tourner le moteur de navigation et les alarmes
sans interface graphique. Voir [apps/daemon/README.md](../apps/daemon/README.md) :
paquet `nautikd_<version>_arm64.deb`, démarrage automatique,
redémarrage après panne et API protégée par jeton.

## 2. Premier lancement

Au premier lancement, NautiK démarre en **mode simulateur** : un bateau
fictif dans la passe de Tiputa (Rangiroa). Tout ce qui vient du
simulateur est marqué **SIMULATION** ou **POSITION SIMULÉE**. Les traces
enregistrées en simulation sont préfixées « SIMULATION — ».

L'écran se compose :

- d'une **barre d'en-tête** : onglets, pastilles d'état (Position,
  Liaison, AIS, Moteur, Alarmes), heure locale avec son décalage (l'heure
  UTC s'affiche en infobulle), thème jour / nuit et plein écran (bouton,
  F11 ; Échap pour sortir). L'en-tête reste affiché en plein écran ;
- de quatre onglets : **Carte**, **Instruments**, **Météo**,
  **Paramètres**.

Les alertes de position, de liaison et d'alarme restent visibles dans
tous les onglets et en plein écran.

## 3. Cartes

NautiK affiche les cartes raster au format **MBTiles** (un fichier par
carte), lues dans un dossier :

- **Paramètres → Cartes → Choisir un dossier…** : n'importe quel dossier,
  par exemple `Documents\Charts` ou une clé USB ; le choix est conservé ;
- **Relire le dossier** après y avoir copié ou retiré des cartes ;
- **Dossier par défaut** : `%APPDATA%\io.github.laurentpellet.nautik\charts`
  sous Windows ;
- la variable d'environnement `NAUTIK_CHARTS_DIR`, si elle est définie,
  impose le dossier (il n'est alors pas modifiable dans l'interface).

Un dossier choisi qui a disparu (clé USB retirée) est signalé, jamais
remplacé en silence. Un fichier illisible ou corrompu est signalé sans
empêcher le chargement des autres.

**Cartes ENC (S-57)**. **Paramètres → Cartes → Cartes
ENC → Choisir le dossier ENC…** indique où se trouve votre jeu d'échange
ENC (en général le dossier `ENC_ROOT`, avec `CATALOG.031` et les
cellules `.000`). NautiK affiche le nombre de cellules trouvées, celles
qui ont des mises à jour (`.001`, `.002`…) et la présence du catalogue.
Dans la liste **Cellules**, **Lire** décode une cellule et en affiche le
résumé : édition, date de publication, usage (port, approche…),
échelle, nombre d'objets et de sondes, emprise et principales classes
d'objets. Seules les cellules en WGS 84 sont acceptées. Les mises à
jour (`.001`, `.002`… à côté de la cellule) sont appliquées dans
l'ordre ; le résumé indique combien l'ont été et la date de la
dernière. Une mise à jour manquante ou incohérente n'est jamais
appliquée à moitié : la cellule est signalée « pas à jour », avec la
raison. Une cellule annulée par son producteur est signalée en rouge :
ne l'utilisez plus, procurez-vous la cellule qui la remplace. Un objet
incohérent est écarté et signalé, jamais
deviné. Un
dossier ENC disparu est signalé ; **Retirer** l'oublie sans toucher aux
fichiers.

Les cellules ENC de la zone affichée sont dessinées sur la carte, par
dessus les cartes raster, avec une symbologie simplifiée inspirée de la
norme S-52 (ce n'est pas un ECDIS) : terre en beige, zones de
profondeur en bleu d'autant plus soutenu que le fond est faible
(isobathes 2, 10 et 30 m ; l'isobathe de 10 m est plus épaisse), zones
non hydrographiées en gris, sondes en mètres à partir du zoom 12 (en
gras sous 10 m), dangers (épaves, obstructions, roches) en magenta,
bouées et balises à leur couleur, feux en halo magenta. Les cellules
les plus détaillées sont dessinées par-dessus ; 16 au plus à la fois.
Le calque **Cartes ENC (S-57)** du menu **Calques** les masque. Une
note en haut à droite de la carte signale toujours ce qui n'est pas
affiché ou pas fiable : cellule pas à jour, cellule annulée (jamais
dessinée), cellule illisible, trop de cellules dans la vue (« zoomez »).
Après avoir ajouté des cellules ou des mises à jour, **Relire le
dossier** redessine la carte.

**Base CM93 (C-MAP).** Une base CM93 v2 (dossier contenant
`CM93OBJ.DIC` et `CM93ATTR.DIC`) placée dans le dossier ENC, ou dans un
de ses sous-dossiers sur deux niveaux, est reconnue : le panneau
**Paramètres → Cartes** indique « Base CM93 trouvée ». Ses cartes sont
dessinées **sous** les ENC officielles, avec la même symbologie, de la
moins à la plus détaillée selon le zoom (16 cellules au plus). Le
calque **Cartes CM93** du menu **Calques** les masque, indépendamment
des ENC : on peut ainsi afficher les ENC seules, la CM93 seule ou les
deux. Ce ne
sont pas des cartes officielles et elles sont souvent anciennes : la
note de la carte le rappelle toujours, avec la date des données. Elle
signale aussi le décalage WGS 84 déclaré par certaines cellules (il
n'est pas corrigé, comme dans OpenCPN : vérifiez la position par
rapport à la côte), les cellules illisibles et celles qui ne sont pas
dessinées (« zoomez »). Certaines copies de bases CM93 contiennent un
double complet de la base dans un sous-dossier (par exemple
`00300420/`) : il est ignoré et peut être supprimé par vous pour
gagner de la place.

Sur la carte :

- molette ou pincement : zoom ; clic droit glissé : rotation.

**Barre d'outils de la carte** (en bas, icônes avec infobulles) :

- flèche **Nord en haut** (rotation libre quand elle est relâchée) et
  viseur **Centrer sur le bateau** (inactif sans position) ;
- menu **Calques ▾** (icône des calques) : groupe **Cartes** (cartes
  MBTiles quand plusieurs sont chargées, la plus détaillée en tête, puis
  **Cartes ENC (S-57)** et, si une base est trouvée, **Cartes CM93**),
  puis, après un séparateur, routes, traces et cibles AIS. Le menu se ferme par Échap ou un clic ailleurs. Une carte ou
  un calque masqués sont rappelés en haut à droite de la carte (« Masqué :
  … ») et par une pastille sur le bouton ; une carte seule est toujours
  affichée ;
- **Deux cartes côte à côte** : partage la carte en deux volets (l'un
  au-dessus de l'autre dans une fenêtre étroite). Le volet de droite a son
  propre choix de cartes, ENC et CM93 comprises (étiquette « Droite :
  … » en haut du volet, par exemple « Droite : CM93 » pour comparer la
  CM93 aux ENC affichées à gauche ; choix mémorisé) et
  affiche les mêmes objets que celui de gauche (bateau, routes, traces,
  AIS, mouillage) ;
- cadenas **Verrouiller les deux cartes** (visible avec les deux volets,
  verrouillé par défaut) : même zoom, même position et même orientation
  dans les deux volets ; le volet que vous déplacez entraîne l'autre.
  Cadenas ouvert : chaque volet est libre. Au verrouillage, le volet de
  droite rejoint celui de gauche ;
- métriques de base : SOG, COG, CAP, FOND et position. Une valeur
  obsolète est estompée et marquée ⚠ ; sans position, « Aucune position »
  est affiché ;
- icônes **Panneau**, **Mouillage** (ancre), **Navigation**
  (cadran) : affichent ou masquent chaque panneau (estompée = masqué) ;
- **↺ Disposition par défaut** : remet tous les panneaux en place et les
  réaffiche.

**Panneaux déplaçables.** Chaque panneau (y compris la barre) se déplace
par sa poignée ⠿ (souris, doigt ou flèches du clavier) et se masque par
son **×**. Double-clic sur la poignée : retour à sa place. Une barre
masquée se rappelle par le bouton **▴ Barre**, en bas à gauche. La
disposition est conservée d'un lancement à l'autre ; un panneau ne peut
pas sortir de la fenêtre. Masquer un panneau ne masque jamais une alarme :
le bandeau d'alarme, les pastilles d'en-tête et les alertes de position
et de liaison restent toujours visibles, et la surveillance du mouillage
continue même si son panneau est masqué.

## 4. Sources de données

**Paramètres → Source → Modifier la source.** Une seule source est
active à la fois ; il n'y a jamais de bascule silencieuse vers une autre.

| Mode | Usage |
| --- | --- |
| Simulateur | démonstration, essais à terre |
| Signal K | serveur Signal K du bord (OpenPlotter, etc.) : adresse et, si besoin, jeton d'accès |
| NMEA 0183 — TCP | multiplexeur ou passerelle Wi-Fi (adresse et port, souvent 10110) |
| NMEA 0183 — UDP | diffusion réseau (port d'écoute) |
| NMEA 0183 — Port série (USB) | GPS ou multiplexeur raccordé en USB/série (port et débit, souvent 4800 ou 38400) |
| Rejeu d'un enregistrement | relecture d'un fichier NMEA enregistré ; traité comme une simulation (**REJEU — DONNÉES NON ACTUELLES**) |

Le jeton Signal K est rangé dans le coffre du système (Gestionnaire
d'identification sous Windows), jamais en clair.

**Priorités.** Si plusieurs équipements fournissent la même donnée (deux
GPS par exemple), indiquez leur ordre de préférence (identifiants ou
préfixes, ex. `nmea0183.GP`). NautiK garde la source retenue tant qu'elle
reste fraîche, puis passe à la suivante.

**Ce que NautiK ne fait jamais :**

- inventer une position : une position que l'équipement déclare
  invalide, estimée ou simulée est ignorée ;
- masquer une perte : liaison coupée, données obsolètes (position de plus
  de 5 s) ou moteur muet sont affichés en clair ;
- émettre quoi que ce soit vers vos instruments : NautiK est en lecture
  seule (aucune phrase NMEA ni commande NMEA 2000 envoyée).

Les phrases rejetées (somme de contrôle fausse, valeurs hors bornes,
débit excessif au-delà de 200 phrases par seconde) sont comptées et la
dernière cause est affichée.

## 5. Instruments

L'onglet **Instruments** regroupe vitesse fond et surface, route, cap,
profondeur, vent apparent (cadran bâbord / tribord et tendance sur 1 h),
navigation active, position, ainsi que moteur, énergie, réservoirs et
environnement lorsque ces données sont reçues. Une donnée obsolète est
estompée et signalée ; une donnée absente reste vide.

Les **Données historiques** (15 min, 1 h, 6 h) sont conservées en mémoire
depuis le lancement. Les unités se règlent dans **Paramètres → Unités**.

## 6. Routes, navigation et traces

- **Nouvelle route** : cliquez les points de passage sur la carte, puis
  **Enregistrer**.
- **Naviguer** : active la route ; NautiK affiche la distance et le
  relèvement du prochain point, l'écart de route (XTE), la distance
  restante et l'heure d'arrivée estimée. Un point est considéré atteint à
  0,1 mille. **Arrêter la navigation** la désactive.
- Une route peut être inversée ou supprimée.
- **Traces** : **Enregistrer** démarre l'enregistrement (un point toutes
  les 5 s au plus, et seulement après 10 m parcourus). Une trace
  interrompue par un arrêt brutal est clôturée au redémarrage, à l'heure
  de son dernier point.
- **GPX** : import et export des routes, points et traces. Un fichier
  dont une coordonnée est invalide est refusé en entier.

## 7. AIS

Les navires AIS reçus apparaissent sur la carte : triangle orienté
(classe A plein, classe B différencié), vecteur de déplacement à 6 min,
historique en pointillés. Une cible obsolète est estompée, une cible
perdue grisée, puis retirée après 30 min de silence. Cliquez une cible
pour afficher son nom, sa vitesse, sa route et son état.

NautiK suit jusqu'à 2 000 cibles. Le calcul du point de rapprochement
(CPA/TCPA) n'est pas encore disponible.

## 8. Alarmes

Deux alarmes sont évaluées par le moteur, même sans interface :

- **Mouillage** : carte **Mouillage → Mouiller ici** (position récente
  requise), rayon de 10 à 2 000 m et temporisation. Le cercle d'évitage
  est tracé sur la carte. **Lever l'ancre** demande confirmation.
- **Perte de position** : position absente ou obsolète au-delà de la
  temporisation (5 s par défaut), dès qu'une première position a été
  reçue ou que le mouillage est surveillé.

Une alarme déclenchée affiche un bandeau rouge et sonne jusqu'à
l'**acquittement** ; acquittée, elle reste affichée tant que la situation
n'est pas rétablie. Désactiver une alarme (**Paramètres → Alarmes**)
demande confirmation et reste visible. L'historique des alarmes y est
consultable.

Le son est produit par l'ordinateur : vérifiez le volume, et que la mise
en veille ne coupe pas l'application.

## 9. Météo (vent)

L'onglet **Météo** affiche le vent prévu à 10 m à partir de fichiers
GRIB2 :

- **Importer un fichier GRIB…** : fichier obtenu par ailleurs (courriel,
  satellite, clé USB) ;
- **Télécharger — GFS 0,25° (NOAA)** : uniquement quand vous le
  demandez, pour la zone affichée.

Les fichiers sont conservés hors ligne. Une prévision ancienne ou une
échéance passée est signalée. Une prévision n'est pas un bulletin
officiel.

## 10. Données et sauvegarde

Les routes, traces, réglages et l'historique des alarmes sont dans la
base `nautik.db` du dossier de données de l'application (pour le service
`nautikd` : `/var/lib/nautik`). Sauvegardez ce dossier application
arrêtée. Après un arrêt brutal, la base est vérifiée au démarrage du
service ; une anomalie est signalée dans le journal.

## 11. Performances mesurées

Mesures de référence (outil `scripts/measure-nautikd.ps1` ou `.sh`,
service `nautikd` alimenté par un multiplexeur simulé) : voir la section
« Mesures » de [TASKS.md](../TASKS.md), phase 8. Les valeurs du Raspberry
Pi restent à relever.

## 12. Dépannage

| Symptôme | Vérifications |
| --- | --- |
| « Aucune position » | source choisie, câble ou réseau, état de la liaison dans l'en-tête, phrases rejetées dans Paramètres → Source |
| Liaison « déconnectée » | adresse et port, multiplexeur allumé ; NautiK réessaie seul (1 s, puis jusqu'à 30 s d'intervalle) |
| « Moteur muet » | l'application ne reçoit plus le moteur de navigation : redémarrez-la |
| Pas de carte | fichiers `.mbtiles` dans le bon dossier, application relancée |
| Port série refusé (Linux) | utilisateur membre du groupe `dialout` |
