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
  Liaison, AIS, Moteur, Alarmes), heure UTC, thème jour / nuit et plein
  écran (bouton, F11 ; Échap pour sortir) ;
- de quatre onglets : **Carte**, **Instruments**, **Météo**,
  **Paramètres**.

Les alertes de position, de liaison et d'alarme restent visibles dans
tous les onglets et en plein écran.

## 3. Cartes

NautiK affiche les cartes raster au format **MBTiles** (un fichier par
carte). Copiez vos fichiers `.mbtiles` dans le dossier des cartes puis
relancez l'application :

- Windows : `%APPDATA%\io.github.laurentpellet.nautik\charts` ;
- ou le dossier indiqué par la variable d'environnement
  `NAUTIK_CHARTS_DIR`.

Le chemin exact est affiché tant qu'aucune carte n'est chargée. Un
fichier illisible ou corrompu est signalé sans empêcher le chargement des
autres.

Sur la carte :

- molette ou pincement : zoom ; clic droit glissé : rotation ;
- bouton **Nord en haut** : remet la carte droite ;
- menu **Calques ▾** : affiche ou masque cartes, routes, traces, AIS,
  vent ; un calque masqué reste signalé.

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
