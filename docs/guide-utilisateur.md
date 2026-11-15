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
Le bateau simulé reste immobile (SOG 0, COG « — ») tant qu'aucune route
n'est active ; il suit la route active à la vitesse du simulateur. Sa
vitesse surface est la vitesse fond à ±4 % près (pas de courant simulé).
Son angle de barre suit la giration (barre en butée à 35° dans un virage
franc, droite à l'arrêt).

L'écran se compose :

- d'une **barre unique en haut**, en trois groupes séparés par un trait :
  les onglets en icônes (carte, cadran pour Instruments, curseurs pour
  Paramètres ; nom en infobulle) ; sur l'onglet Carte, SOG et force du vent
  réel (« — » si absent, valeur signalée si obsolète)
  suivis des outils de la carte, terminés par le bouton **Homme à la mer**
  (bouée rouge) ; puis l'icône **État des données**
  (tracé de pouls), de la couleur de l'état le plus grave : verte si tout
  va bien, orange en simulation ou en attente, rouge sur fond rouge en
  alerte (résumé en infobulle). Un clic ouvre ou ferme le panneau d'état
  (choix mémorisé) : Position (viseur), Liaison (antenne), AIS (triangle),
  Alarmes (cloche) et Moteur (puce), chacun avec son icône colorée, son
  état et le détail en infobulle. Panneau fermé, une alerte reste visible
  par l'icône rouge et le compteur **⚠ N alertes actives** ; puis, tout à
  droite, l'heure
  locale avec son décalage (l'heure UTC s'affiche en infobulle) et les
  icônes thème jour / nuit et plein écran (F11 ; Échap pour sortir). Sur
  un écran pas assez large, le dernier groupe passe sur une seconde ligne.
  La barre reste affichée en plein écran. Au lancement, la fenêtre
  s'ouvre maximisée ;
- de trois onglets : **Carte**, **Instruments** et **Paramètres**.

L'onglet **Paramètres** est découpé en sept sections, choisies dans la
colonne de gauche (ou les onglets du haut sur un écran étroit, flèches du
clavier possibles) : **Général** (unités, couleur de la route active,
vitesse du simulateur), **Instruments** (valeurs de la barre Navigation
et leur ordre), **Source** (données du bord), **Alarmes**
(surveillance et historique), **Cartes** (MBTiles, ENC, îles et
amers), **Météo** et **Journal**. La dernière section ouverte est retrouvée au prochain passage.

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

**Îles et amers (OpenStreetMap).** **Paramètres → Cartes → Îles et
amers** propose onze régions du Pacifique (Polynésie française,
Nouvelle-Calédonie, Wallis-et-Futuna, Fidji, Samoa, Tonga, îles Cook,
Vanuatu…). **Télécharger** récupère, par Internet et à la demande
seulement (service Overpass, jusqu'à quelques minutes), les noms des
archipels, îles et îlots, les passes, balises, bouées, feux, amers,
mouillages et ports de la région ; ils sont ensuite enregistrés et
affichés hors ligne. **Mettre à jour** remplace les données de la région
(en cas d'échec, les précédentes sont gardées) ; **✕** les supprime après
confirmation. Sur la carte, les noms apparaissent selon le zoom
(archipels de loin, îlots de près), les passes en violet, les balises et
bouées à la couleur de leur catégorie (bâbord rouge, tribord vert,
cardinales jaunes…), les feux en jaune avec leurs caractéristiques. Le
calque **Îles et amers (OSM)** du menu **Calques** les masque. Ce sont des
données collaboratives non officielles (© contributeurs OpenStreetMap,
licence ODbL) : elles complètent les cartes marines sans les remplacer.

Sur la carte :

- molette ou pincement : zoom ; clic droit glissé : rotation.

**Outils de la carte** (dans la barre du haut, après les onglets ; icônes
avec infobulles ; ils ne se déplacent pas et ne se masquent pas) :

- flèche **Nord en haut** (rotation libre quand elle est relâchée) et
  viseur **Centrer sur le bateau** (inactif sans position) ;
- menu **Calques ▾** (icône des calques) : groupe **Cartes** (cartes
  MBTiles quand plusieurs sont chargées, la plus détaillée en tête, puis
  **Cartes ENC (S-57)** et, si une base est trouvée, **Cartes CM93**),
  puis, après un séparateur, routes, traces, cibles AIS et îles et
  amers. Le menu se ferme par Échap ou un clic ailleurs. Une carte ou
  un calque masqués sont signalés par une pastille sur le bouton, dont
  l'infobulle en donne la liste (« Calques — masqué : … ») ; une carte seule est toujours
  affichée ;
- bateau **Positionner le bateau ici** : avec la source **Simulateur**
  seulement, le clic suivant sur la carte y place le bateau simulé (Échap
  pour annuler) ; s'il suit une route, il repart vers le même waypoint.
  Avec une source réelle ou un rejeu, l'outil est grisé et le moteur
  refuse : une position réelle n'est jamais remplacée. Le déplacement
  n'est pas mémorisé (au redémarrage, le bateau repart de son point par
  défaut) ;
- règle **Mesurer** : cliquez des points sur la carte ; un panneau en
  haut à gauche donne la distance totale, le dernier segment, son
  relèvement vrai et la durée à la vitesse prévue du plan de route
  (valeurs calculées par le moteur). **Annuler le point**, **Effacer** ;
  Échap, **✕** ou un nouveau clic sur la règle ferme la mesure. Indisponible
  pendant le tracé d'une route ;
- **clic droit** sur la carte (sans glisser ; le glisser droit tourne la
  carte) : menu avec la position visée et, en dessous, sa distance et son
  relèvement vrai depuis le bateau (calculés par le moteur ; « Position du
  bateau inconnue ou obsolète » sans position fraîche), puis
  **Naviguer jusqu'ici** (crée la route directe « Direct HH:MM UTC » du
  bateau à ce point et l'active ; impossible sans position fraîche ou
  pendant le tracé d'une route), **Bateau ici** et **Mesurer depuis ici** ;
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
- icônes **Panneau**, **Mouillage** (ancre), **Navigation** (cadran),
  **Cibles AIS** (triangle), **Route** (tracé) et **Barre de route** :
  affichent ou masquent chaque panneau (estompée = masqué). Le panneau
  **Cibles AIS** est fermé par défaut.

**Barre Navigation.** Le panneau **Navigation** est une barre fixe à côté
de la carte, comme la barre du haut (la carte se réduit d'autant : rien
n'est recouvert). **Clic droit** (ou appui long) sur la barre : **À
gauche** (colonne à gauche de la carte et de la barre de route), **En
bas** (par défaut, sur toute la largeur) ou **À droite** ; Échap ferme le
menu sans rien changer ; choix mémorisé. Elle affiche le compas, puis, par défaut, SOG,
cap, COG, vent apparent, fond, vitesse surface, angle du vent et angle de
barre (**BARRE**, bord Td / Bd), tous en très grands caractères. Dans
**Paramètres → Instruments**, cochez les valeurs à afficher et changez
leur ordre avec les flèches ↑ ↓ (au moins une reste affichée ; **Tout
afficher** revient au choix par défaut ; choix mémorisé) (VMG et autre amure ne sont plus affichés en
valeurs ; les laylines restent sur la carte). Elle est toujours affichée
au lancement ; l'icône **Navigation** de la barre d'outils la masque
jusqu'au prochain lancement. Son bouton **«** (**»** à droite), en haut à gauche comme sur la barre
de route et le même
que celui de la barre de route, la **réduit** : le compas disparaît, les mêmes
valeurs restent, présentées comme la barre de route réduite (nom en haut, valeur, unité dessous) ; l'autre flèche la déplie (choix mémorisé). Une valeur obsolète est estompée et marquée ⚠ ; une
donnée absente reste « — ».

**Panneaux déplaçables.** Les autres panneaux se déplacent par leur poignée ⠿
(souris, doigt ou flèches du clavier) et se masquent par leur **×**.
Double-clic sur la poignée : retour à sa place. La
disposition est conservée d'un lancement à l'autre ; un panneau ne peut
pas sortir de la fenêtre. Masquer un panneau ne masque jamais une alarme :
le bandeau d'alarme, l'icône État des données, le compteur d'alertes et les alertes de position
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
angle de barre (phrase NMEA 0183 RSA ou chemin Signal K
`steering.rudderAngle` ; au-delà de 90° d'un bord, la valeur est rejetée),
profondeur, vent apparent (cadran bâbord / tribord et tendance sur 1 h),
navigation active, position, ainsi que moteur, énergie, réservoirs et
environnement lorsque ces données sont reçues. Une donnée obsolète est
estompée et signalée ; une donnée absente reste vide.

Les **Données historiques** (15 min, 1 h, 6 h) sont conservées en mémoire
depuis le lancement. Les unités se règlent dans **Paramètres → Général → Unités**.

## 6. Routes, navigation et traces

Routes et traces partagent le panneau **Routes et traces** (à droite de
la carte, sous Mouillage et Navigation ; la colonne défile si la fenêtre
est basse), avec trois onglets **Routes**, **Traces** et **Marques** (le dernier
onglet choisi est mémorisé).

- **Nouvelle route** : cliquez les points de passage sur la carte. Le
  premier point s'appelle « Départ » (vert), le dernier est marqué en
  rouge ; les noms des points s'affichent sur la carte. Pendant
  l'édition :
  - **+ Départ** / **+ Arrivée** : le prochain clic ajoute un point en
    tête ou en fin de route ; **↳** après un point insère le clic suivant
    juste après lui ;
  - champs **Rechercher le départ** / **Rechercher l'arrivée** (sous
    l'éditeur) : même recherche que la loupe (coordonnées, marques,
    waypoints, îles, amers) ; le lieu choisi remplace le départ ou
    l'arrivée (ou s'ajoute tant que la route a moins de deux points), avec
    son nom ;
  - un point se **déplace en le glissant** sur la carte ;
  - chaque point se **nomme** dans la liste (vide : nom automatique
    WP001…) et se supprime par **✕** ; au-dessus de chaque point, la
    longueur et le relèvement du segment qui y arrive (« … » pendant le
    recalcul) ;
  - **Enregistrer** garde la route, **Annuler** abandonne les changements.
- **Modifier** reprend une route enregistrée dans l'éditeur (impossible
  pour la route en cours de navigation).
- **Plan de route** : pour la route sélectionnée ou en édition, NautiK
  calcule (moteur) la longueur, la durée à la vitesse prévue (réglable,
  dans l'unité choisie) et, si une heure de départ UTC est donnée
  (**Maintenant** ou saisie), l'heure d'arrivée à chaque point et en fin
  de route. Tableau par branche : distance, relèvement, durée, cumul.
  Ces estimations supposent une vitesse constante et ne tiennent compte
  ni du vent ni du courant.
- **Zoomer** : la carte se cadre sur l'emprise de la route (y compris de
  part et d'autre de l'antiméridien). Sélectionner une route ou afficher
  une trace cadre aussi la carte dessus.
- **Naviguer** : active la route, dessinée sur la carte en jaune bordé
  d'un liseré sombre (couleur réglable dans **Paramètres → Général →
  Carte → Couleur de la route active**, bouton **Jaune (défaut)** pour revenir ;
  les autres routes restent violettes ; décocher le calque **Routes**
  masque les autres routes, jamais la route active) ; NautiK affiche la distance et le
  relèvement du prochain point, l'écart de route (XTE), la distance
  restante et l'heure d'arrivée estimée. Un point est considéré atteint à
  0,1 mille. **Arrêter la navigation** la désactive.
- **Sur la carte**, chaque segment de route porte de petites flèches
  blanches indiquant le sens de parcours (routes enregistrées et route en
  cours de tracé) ; pour la route active, la longueur de chaque segment
  (calculée par le moteur, dans l'unité choisie) s'affiche en italique au
  milieu du segment.
- **Barre de route** : pendant la navigation, une barre latérale s'affiche
  collée à gauche de la carte (la carte se décale d'autant). **Clic
  droit** (ou appui long) sur la barre : **À gauche de la carte** ou **À
  droite de la carte** (choix mémorisé). Son bouton **«** (**»** à
  droite) la réduit à une colonne étroite qui garde le rang du waypoint,
  DTW et BTW ; l'autre flèche la déplie (choix mémorisé). Elle rappelle la route,
  le waypoint visé (rang / nombre de points) et la mention SIMULATION le
  cas échéant, puis :
  - **DTW** : distance au waypoint ; **BTW** : son relèvement vrai ;
  - **XTE** : écart de route (D : à droite, G : à gauche) ;
  - **TEMPS WP** et **ETA WP** : temps restant et heure locale d'arrivée au
    waypoint (l'unité donne le décalage, par exemple UTC−10) ;
  - **RESTE** : distance jusqu'à la fin de la route ;
  - **DURÉE** et **ETA FIN** : temps restant et heure locale d'arrivée en
    fin de route.

  Les temps et heures sont estimés à la vitesse fond courante ; vitesse
  nulle ou inconnue : « — ». Si la position est obsolète ou absente, ou
  si le moteur ne répond plus, toutes les valeurs sont estompées et
  marquées ⚠ (l'infobulle donne l'âge du calcul). Ses boutons affichent
  ou masquent le panneau Route et arrêtent la navigation. L'icône
  **Barre de route** de la barre d'outils la masque ou la réaffiche.

- Une route peut être inversée ou supprimée.
- **Traces** (onglet **Traces**) : **Enregistrer** démarre
  l'enregistrement (un point toutes les 5 s au plus, et seulement après
  10 m parcourus). Le bouton **lecture / arrêt** de la barre d'outils
  fait de même sans ouvrir le panneau : triangle pour démarrer, carré
  rouge pendant l'enregistrement pour l'arrêter. Une trace interrompue par un arrêt brutal est clôturée
  au redémarrage, à l'heure de son dernier point. Pour une trace
  affichée : longueur, durée (du premier au dernier point), **Zoomer** et
  **Renommer**.
- **Relire** (onglet **Traces**) : statistiques de la trace calculées par
  NautiK (distance, durée totale et en route, vitesses moyenne et
  maximale, profondeur minimale), trace colorée selon la vitesse (bleu à
  l'arrêt → rouge au-delà de 9 nœuds ; gris si la vitesse est inconnue),
  curseur et lecture accélérée (×10 à ×30 min/s) avec l'heure, la vitesse,
  la distance parcourue et la profondeur enregistrées au point courant.
- **GPX** : import et export des routes, points et traces. Un fichier
  dont une coordonnée est invalide est refusé en entier.

### Marques

L'onglet **Marques** du panneau Routes et traces regroupe vos points
personnels (repère, mouillage, danger, plongée, pêche, carburant,
information), avec une note libre :

- **clic droit sur la carte → Marque ici**, ou **+ Au bateau** (position
  récente requise) ; nom, type et note se règlent avant **Enregistrer** ;
- cliquer le nom d'une marque cadre la carte dessus ; ✎ la modifie, ✕ la
  supprime (avec confirmation) ;
- **Importer KML** / **Exporter KML** : échange avec Google Earth,
  OpenCPN, Navionics… Seuls les points sont importés ; les lignes et
  surfaces sont écartées et signalées. Un fichier invalide est refusé en
  entier ;
- le calque **Marques** les affiche ou les masque.

### Recherche d'un lieu

La loupe de la barre d'outils cherche, sans Internet, dans vos marques,
waypoints, points de routes et îles et amers téléchargés (sans tenir
compte des majuscules ni des accents). Elle accepte aussi des
coordonnées : `-17.5 -149.6`, `17°32,1'S 149°34,2'W`,
`S 17 32.1 O 149 34.2`… Choisir un résultat cadre la carte dessus.

### Aides à la navigation

- **Vecteur et ligne de cap** (calque) : la route fond prévue à 6 min et
  le cap du bateau, tracés à partir de données récentes seulement.
- **Laylines** : avec le vent réel, le
  calque **Laylines** trace les laylines du waypoint visé (45° au près,
  150° au portant). Ce sont des repères géométriques : ils ne tiennent
  compte ni des polaires ni du courant.
- **VRM/EBL** (bouton de la barre d'outils) : cercle de distance et ligne
  de relèvement autour du bateau. Un clic sur la carte les fait passer
  par ce point ; rayon et relèvement se règlent aussi à la main. Sans
  position récente, rien n'est tracé.

### Journal de bord

La section **Journal** des **Paramètres** (icône **Journal de bord** de la
barre d'outils) liste les entrées, de la plus récente à la plus
ancienne :

- **Ajouter** : entrée libre (ris pris, moteur, mouillage…), complétée
  automatiquement par l'heure UTC, la position et les valeurs du bord ;
- **Relevés automatiques** : toutes les 15 min, 30 min, 1 h (défaut) ou
  4 h, à heure ronde, même sans interface (service `nautikd`) ;
- seules des valeurs récentes sont reprises ; une position ancienne est
  gardée mais marquée **obsolète**, une donnée absente s'affiche « — » et
  les entrées prises en simulation sont marquées ;
- **Exporter CSV** : fichier pour tableur (séparateur `;`, nœuds,
  degrés, hPa).

## 7. AIS

Les navires AIS reçus apparaissent sur la carte : triangle orienté
(classe A plein, classe B différencié), vecteur de déplacement à 6 min,
historique en pointillés. Une cible obsolète est estompée, une cible
perdue grisée, puis retirée après 30 min de silence. Cliquez une cible
pour afficher son nom, sa vitesse, sa route et son état.

L'icône **Cibles AIS** (triangle) de la barre d'outils ouvre le panneau
des cibles : nom ou MMSI, vitesse, route, état (obsolète, perdue) et âge
de chaque cible, les actives en premier. Un clic sur une ligne la
sélectionne sur la carte. Une erreur de réception AIS est signalée dans
ce panneau, ou dans le panneau latéral quand il est fermé.

NautiK suit jusqu'à 2 000 cibles. Le calcul du point de rapprochement
(CPA/TCPA) n'est pas encore disponible.

## 8. Alarmes

Trois alarmes sont évaluées par le moteur, même sans interface :

- **Homme à la mer (MOB)** : bouton bouée rouge en fin de barre de la carte (juste à gauche de l'état
  des données) ou
  **Ctrl+M**. Le point est posé à la dernière position reçue ; si elle
  est ancienne, il est marqué **approximatif** ; sans aucune position, le
  MOB est refusé (notez l'heure et le cap). Un panneau affiche en grand,
  dans tous les onglets, le relèvement et la distance au MOB et le temps
  écoulé ; la carte montre le marqueur et la ligne bateau → MOB. **Couper
  l'alarme** arrête le son ; **Fin du MOB** (confirmée) termine l'alerte.
  Le MOB survit à un redémarrage et ne peut pas être désactivé.

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

La section **Météo** des **Paramètres** (icône **Météo** de la barre
d'outils) affiche le vent prévu à 10 m à partir de fichiers
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
