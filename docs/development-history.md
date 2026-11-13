# Atelier — prototype de terminal macOS

Prototype exploratoire en Rust, egui/eframe et Alacritty via egui_term. L’objectif est de tester le confort d’un terminal organisé par projets avant de stabiliser l’architecture.

## Lancer

Double-cliquer sur `dist/Atelier.app`, ou depuis ce dossier :

```sh
sh scripts/cargo.sh run --release --locked
```

Rust est installé localement dans `.toolchain`, sans modification du profil shell. Avec une installation Rust classique, les scripts utilisent `cargo` du PATH.

## Utiliser

- **⌘O / Fichier → Ouvrir un projet…** : ouvrir un dossier, chemin absolu ou `~/…`. Suggestions de dossiers pendant la saisie ; flèches haut/bas pour choisir, Tab ou clic pour compléter, Entrée pour ouvrir le chemin saisi.
- **⌘D / Terminal → Diviser le terminal** : ajouter un shell dans le répertoire courant du panneau actif, sans limite de deux panneaux. La grille s’adapte au nombre de panneaux.
- Cliquer dans un panneau pour y écrire. Les projets conservent leurs shells pendant que l’application reste ouverte ; après fermeture, de nouveaux shells sont démarrés dans les dossiers sauvegardés.
- **⌘, / Atelier → Réglages…** : thème et taille du texte ; **Échap** pour fermer.
- `exit` ferme le panneau si un autre existe : le panneau restant reprend toute la largeur et le focus. Le dernier panneau reste affiché avec « Relancer le shell ».
- SSH, tmux et Docker s’utilisent comme commandes normales, s’ils sont installés/configurés. L’application n’installe ni tmux ni Docker, et ne se connecte à aucun serveur automatiquement.

## Construire une application

```sh
sh scripts/bundle.sh
open dist/Atelier.app
```

Le bundle est signé ad hoc pour un usage local, pas notarisé pour distribution. La compilation produit l’architecture de la machine (Apple Silicon ici).

## Limites du prototype

Disposition en grille automatique, sans redimensionnement manuel des séparateurs, projets et disposition restaurés au redémarrage avec de nouveaux shells. Fermer l’application termine les shells locaux ; utiliser tmux sur le serveur pour conserver les processus distants. Pas encore de gestionnaire SSH/Docker, de confirmation avant fermeture, de recherche, ni de restauration des processus en cours. Ne pas y laisser pour l’instant de travail critique non sauvegardé.

Le widget egui_term est expérimental : la compatibilité complète avec les éditeurs, les méthodes de saisie et les applications terminal reste à vérifier. Ce prototype ne constitue pas une promesse de performances supérieures aux terminaux existants. Le rendu est déclenché par les événements, sans boucle volontaire de rafraîchissement permanent.

## Dépendance adaptée

`vendor/egui_term` contient la version 0.1.0 (MIT), avec deux adaptations : clavier dirigé vers le panneau actif même lorsque le pointeur est ailleurs ; arrêt propre du relais d’événements lors de la déconnexion du canal. Les licences amont sont conservées. Source : https://github.com/kemokempo/egui_term

## Vérification manuelle

1. Écrire une commande avec accents, déplacer la souris dans la barre latérale, continuer à écrire.
2. Diviser, saisir des commandes différentes dans les deux panneaux, redimensionner.
3. Ouvrir un second dossier et revenir au premier : vérifier que les shells restent ouverts.
4. Essayer `printf '\033[32mvert\033[0m\n'`, `top`, sélection/copie/collage, et un défilement de logs.
5. Exécuter `exit`, relancer le shell, puis fermer l’application.
6. Comparer avec Terminal.app : CPU au repos, mémoire et réactivité sous charge, sur la même machine et avec le même shell.

## Vérifications effectuées le 11 novembre 2026

- Compilation `release` réussie sur Apple Silicon ; exécutable arm64 d’environ 6 Mo.
- Bundle `.app` : plist valide et signature ad hoc vérifiée.
- Ouverture de la fenêtre et shell de connexion fonctionnels.
- Exécution de `printf` avec accents dans le premier panneau, création du second avec ⌘D, puis exécution indépendante dans ce panneau : vérifiées visuellement.
- Dialogue d’ouverture de projet affiché avec ⌘O. La validation du dossier et le retour entre projets restent à tester : l’outil de contrôle de fenêtre a cessé d’accepter les actions à cette étape.
- Pas encore de mesure comparative CPU/mémoire/latence, ni de test SSH, tmux ou Docker.

## Apparence claire et sombre

La fenêtre **Réglages**, accessible avec **⌘,** ou le menu **Atelier → Réglages…**, propose **Système**, **Clair** et **Sombre**, ainsi que la taille du texte. Par défaut, l’application suit le thème macOS, y compris ses changements pendant l’exécution. Le fond, le texte, les accents, les erreurs et la palette ANSI du terminal suivent le même thème. Le choix du thème et la taille du texte sont sauvegardés et restaurés au prochain lancement. Les couleurs RGB explicites émises par un programme restent sous son contrôle.

Vérification : compilation release réussie ; lancement sur macOS en mode clair, passage manuel en sombre puis retour à Système clair vérifiés visuellement sans recréer le shell. Le changement du réglage macOS en cours d’exécution n’a pas été manipulé lors de cette vérification.

Réglages : compilation release et signature vérifiées ; panneau ouvert par bouton et par ⌘, sur le clavier français, fermeture avec Échap vérifiée. Les contrôles de thème et de taille ne sont plus dans la barre principale.

Fermeture des splits : compilation release réussie ; scénario ⌘D → exit vérifié visuellement (panneau restant en pleine largeur), puis exit du dernier shell et relance par bouton vérifiés. Tests effectués dans une instance isolée.

Autocomplétion : recherche des dossiers en arrière-plan, jusqu’à 100 suggestions triées ; dossiers cachés proposés si le préfixe commence par un point. Test automatisé réussi (filtrage, espaces, casse, dossiers cachés et chemin inexistant). Parcours visuel validé : `~/Documents/dev/ter` → Tab → `~/Documents/dev/terminal/` → Entrée → shell dans le bon dossier.


## Sauvegarde et fermeture des projets

Les modifications sont sauvegardées automatiquement dans `~/Library/Application Support/Atelier/state.json` : dossiers ouverts, nombre de panneaux et panneau actif par projet, projet sélectionné, thème et taille du texte. Le fichier est remplacé atomiquement. Aucun historique de terminal ni secret SSH n’y est enregistré.

La croix **×** à côté d’un projet ferme ses shells et le retire des projets sauvegardés. Son dossier reste intact. Fermer tous les projets conserve un espace vide au prochain lancement, avec un bouton pour en ouvrir un.

Au redémarrage, les shells sont recréés dans les dossiers des projets, sans restauration des commandes ou processus. Les dossiers indisponibles sont signalés et ignorés ; la sauvegarde n’est pas réécrite au simple chargement, mais une modification ultérieure de l’espace sauvegarde la liste actuellement ouverte. Un fichier invalide est conservé, et la sauvegarde est désactivée pour cette exécution avec une erreur affichée. En cas d’échec d’écriture, corriger les permissions puis relancer. Plusieurs instances peuvent écrire le même fichier : la dernière modification sauvegardée prévaut.

Validation : tests de sérialisation/relecture, espace vide et fichier invalide réussis ; compilation release et signature validées. Le contrôle visuel de cette version n’a pas été autorisé par l’outil. `ATELIER_STATE_FILE` permet de choisir un fichier isolé pour les tests.


## Navigation et zoom du panneau actif

- **⌥⌘↑ / ⌥⌘↓** : projet précédent / suivant.
- **⌥⌘← / ⌥⌘→** : panneau précédent / suivant du projet, dans l’ordre de la grille (ligne par ligne).
- **⌥⌘Entrée** : agrandir le panneau actif dans toute la zone terminal ; un second appui restaure la grille. Les autres shells restent ouverts.
- La navigation boucle aux extrémités. En mode agrandi, changer de panneau affiche le nouveau panneau actif. Ajouter un split quitte le mode agrandi.
- Ces raccourcis sont réservés à l’application et désactivés lorsque le dialogue de projet ou les réglages sont ouverts.
- Le nombre de panneaux et le panneau actif sont sauvegardés ; le zoom est temporaire.

Validation : quatre tests passent, couvrant la navigation circulaire, le zoom/restauration, l’autocomplétion et la sauvegarde/restauration de cinq panneaux. Compilation release et signature vérifiées. Les nouvelles interactions clavier et la grille n’ont pas été vérifiées visuellement, l’accès au contrôle d’Atelier ayant été refusé lors du test précédent.

Le panneau actif est encadré par une bordure accentuée de 2,5 points, adaptée aux thèmes clair/sombre. Les autres panneaux gardent un contour discret. La bordure suit le focus clavier et les clics, y compris en mode agrandi.

**⌥⌘B** affiche ou masque la barre latérale des projets. Le terminal récupère la largeur disponible ; le projet et les shells restent ouverts. Le choix est sauvegardé au prochain lancement. Comme les autres raccourcis de navigation, il est désactivé lorsque les réglages ou le dialogue de projet sont ouverts.

## Menus natifs macOS

Les commandes Ouvrir un projet et Diviser sont dans les menus **Fichier** et **Terminal** de la barre macOS, avec ⌘O et ⌘D. Les réglages sont dans le menu de l’application avec ⌘,. La barre de boutons interne est retirée. Diviser est désactivé sans projet ou pendant un dialogue ; les événements de menu réveillent le rendu sans boucle de scrutation. Menu fourni par muda. Compilation et quatre tests existants réussis ; les menus et leurs raccourcis natifs restent à vérifier visuellement.

Le menu natif **Affichage** contient **Masquer/Afficher la barre des projets** (⌥⌘B) et **Agrandir le panneau actif/Rétablir les panneaux** (⌥⌘Entrée). Les libellés suivent l’état actuel, et le zoom est désactivé sans projet. Les raccourcis sont traités par le menu natif pour éviter un double déclenchement avec egui. Vérification : compilation et tests existants ; pas de validation visuelle du menu.

Le menu **Affichage** propose aussi **Projet précédent/suivant** (⌥⌘↑/↓) et **Panneau précédent/suivant** (⌥⌘←/→). La navigation reste circulaire et fonctionne en mode agrandi. Les commandes sont grisées lorsqu’il n’existe pas d’autre projet/panneau ou lorsqu’un dialogue est ouvert. Les accélérateurs natifs remplacent le traitement egui correspondant pour éviter tout double déclenchement. Compilation et tests de navigation validés ; validation visuelle non effectuée.

**Fichier → Fermer le projet** (**⌘W**) ferme le projet actif et tous ses shells, comme la croix de la barre latérale. Le dossier est conservé et la fermeture est sauvegardée. La commande est désactivée sans projet ou pendant un dialogue ; fermer le dernier projet laisse l’application ouverte sur l’espace vide. Compilation et tests existants vérifiés ; interaction native non vérifiée visuellement.

## Rouvrir un projet fermé

**Fichier → Rouvrir le dernier projet fermé**, **⌘⇧T**, recrée le dernier projet fermé par la croix ou ⌘W. L’historique conserve au plus 10 fermetures (les plus récentes), dans `state.json`, et survit au redémarrage. Chaque entrée contient seulement le dossier, le nombre de panneaux et le panneau actif. Les anciens fichiers de sauvegarde sont compatibles, avec un historique vide.

Aucun shell, PTY ou historique de texte n’est conservé par cette liste. Aucun timer ni processus supplémentaire n’est créé. La réouverture démarre de nouveaux shells et sélectionne le projet restauré ; les commandes précédentes ne reprennent pas. Des appuis successifs rouvrent les projets du plus récent au plus ancien. Une entrée est retirée après réouverture réussie. Si le dossier est indisponible ou si un shell ne démarre pas, l’entrée reste disponible et une erreur est affichée. Les fermetures successives du même dossier constituent des entrées distinctes. Quitter l’application conserve les projets ouverts normalement et ne les ajoute pas à cet historique.

Validation : cinq tests passent, notamment 13 fermetures conservant les 10 dernières, ordre de réouverture après sauvegarde/relecture, disposition des panneaux et compatibilité des anciennes sauvegardes. Build release et signature vérifiés ; le raccourci natif n’a pas été validé visuellement.

## Palette de commandes

**Affichage → Palette de commandes…** (**⌘⇧P**) permet de chercher les projets ouverts, leurs panneaux et les actions des menus : ouvrir/fermer/rouvrir un projet, diviser, réglages, barre latérale, zoom et navigation. Les raccourcis sont indiqués dans les résultats. Les actions sans cible disponible sont omises.

Saisir quelques lettres, choisir avec Haut/Bas puis Entrée, ou cliquer un résultat. Échap, un clic à l’extérieur ou un second ⌘⇧P ferme la palette. La recherche ignore la casse et les accents français courants et accepte des abréviations. Les chemins et titres aident à distinguer les projets homonymes. Un panneau est identifié par son ID de terminal, pas par une position devenue obsolète après un exit.

La palette réutilise les actions du menu natif. Aucun accès disque, réseau, lancement de processus, indexation de fichiers, timer ou dépendance supplémentaire pour la recherche. Les entrées et résultats sont calculés uniquement pendant son affichage, sur la liste locale. La frappe de recherche n’est pas transmise aux shells.

Validation : sept tests passent, dont recherche avec accents/abréviations et interaction egui sans fenêtre native (Haut/Bas, Entrée, Échap, aucun résultat). Compilation release et signature vérifiées. Le rendu macOS et le raccourci natif n’ont pas été vérifiés visuellement.

Correction du raccourci de zoom : `vendor/muda` contient muda 0.17.2 (licences amont conservées), avec une correction ciblée du mapping macOS : `Code::Enter` utilise Return (`\r`), tandis que `Code::NumpadEnter` utilise l’Entrée du pavé numérique (`\u{0003}`). Le raccourci ⌥⌘Entrée garde ainsi la touche principale du MacBook et le menu natif reste l’unique gestionnaire. Un test de régression reproduisait l’erreur avant correction ; les huit tests passent après correction. Compilation release et signature vérifiées ; pas de validation visuelle du raccourci.


## Répertoire propre à chaque panneau

⌘D interroge macOS pour connaître le répertoire du shell local du panneau actif et démarre le nouveau shell dans ce dossier. Chaque panneau mémorise son répertoire séparément. Les répertoires sont relus à la fermeture explicite du projet et à la fermeture normale d’Atelier, avant de sauvegarder. Le redémarrage et ⌘⇧T recréent les shells dans ces dossiers. Une ancienne sauvegarde sans répertoires de panneaux utilise le dossier du projet ; un sous-dossier disparu se replie sur le dossier du projet.

Cette implémentation utilise `proc_pidinfo` à la demande, sans timer, sans lecture périodique et sans modifier zsh. `egui_term` expose le PID de son shell enfant. La requête vise le shell local initial : un `cd` dans SSH, tmux ou un sous-shell distinct n’est pas interprété comme un dossier local. Si macOS ne permet pas de lire le répertoire, le dernier chemin connu est conservé. Après un arrêt forcé/crash, seuls les dossiers déjà enregistrés sont restaurables ; la lecture finale nécessite une fermeture normale.

Validation : test avec un vrai zsh faisant cd dans un dossier avec espaces et accent, relecture des chemins par panneau, migration des anciennes sauvegardes et suite de tests. Le parcours complet dans la fenêtre macOS reste non vérifié visuellement.

## Repère visuel SSH

Chaque en-tête affiche **LOCAL** ou un badge bleu **SSH · alias** (couleurs adaptées au thème). La bordure verte continue d’indiquer le panneau actif. Le badge SSH désigne le processus de connexion en cours, pas une authentification confirmée ; il revient à LOCAL à son retour, y compris après un échec.

Un relais `ssh` privé à Atelier transmet les arguments et le code de retour au véritable exécutable. La fonction `ss` existante, qui appelle `command ssh`, reste inchangée. Pour zsh, des fichiers de démarrage temporaires chargent les fichiers utilisateur puis placent le relais dans PATH après `path_helper`. Aucun fichier utilisateur n’est modifié. Les changements de badge transitent par le PTY avec un jeton propre au panneau ; les titres ordinaires ne les déclenchent pas. Pas de timer ni de scrutation des processus ; un petit processus zsh supplémentaire existe pendant SSH. Les fichiers temporaires sont supprimés à la destruction normale de l’intégration.

Cette première version vise les connexions SSH au premier plan via `ss`/`ssh`. Un appel absolu à `/usr/bin/ssh` contourne le relais. Les sorties redirigées et les commandes de consultation (`-G`, `-V`, `-Q`, `-O`) ne génèrent pas de badge. Les sessions imbriquées, les tâches SSH en arrière-plan et les connexions à l’intérieur de tmux ne sont pas suivies individuellement. L’état SSH n’est pas sauvegardé : les nouveaux shells repartent en LOCAL.

Validation : 12 tests Rust, dont démarrage zsh avec configuration utilisateur réinitialisant PATH ; `python3 tests/ssh_relay.py` vérifie le relais dans un PTY avec un faux SSH (arguments, retour LOCAL après échec, code de sortie, consultation et redirection). Aucun serveur réel contacté. Rendu de la fenêtre non vérifié visuellement.

## Historique de dossiers intégré à Atelier

Au lancement d’un panneau, le backend normalise son répertoire et transmet un `PWD` cohérent au shell, indépendamment du `PWD` hérité par l’application depuis le Finder. Cela évite une résolution supplémentaire par zsh au démarrage ; aucun suivi périodique n’est ajouté. Le test `terminal_starts_zsh_with_stale_inherited_pwd` démarre un vrai zsh dans un PTY avec un `PWD` initial volontairement incorrect.

⌘⇧P recherche les projets ouverts et les dossiers visités, sans installer z, zsh-z, un plugin Oh My Zsh ou un service. Le moteur de classement et la sauvegarde sont écrits en Rust. L’ancienne lecture de `~/.z`, `ZSHZ_DATA` et `_Z_DATA` a été retirée.

Atelier apprend les projets ouverts et les changements de dossier du shell zsh initial. Un petit hook livré dans l’application envoie une notification via le PTY lors d’un changement de dossier ; il utilise uniquement les commandes internes de zsh. Le hook est chargé par les fichiers temporaires de démarrage d’Atelier, sans modifier les fichiers de configuration utilisateur. Il respecte les hooks existants, transmet les chemins physiques (liens symboliques dédupliqués), ignore les sous-shells et ne produit aucune séquence dans une sortie redirigée. Le prompt sert de rattrapage si le changement avait une sortie redirigée, sans répéter un dossier inchangé. Les dossiers distants SSH ne sont pas interprétés comme des chemins locaux.

L’historique garde au plus 2 000 dossiers, avec un compteur de visites et la dernière visite. Un bonus favorise les visites récentes ; les entrées les moins utiles sont évincées au plafond. La palette filtre en mémoire, garde les projets ouverts en priorité et déduplique les chemins. Le classement fréquence/récence départage les correspondances de même pertinence. Le snapshot classé n’est calculé qu’à l’ouverture de la palette ; au plus 100 résultats sont affichés. Aucun scan du disque, thread de lecture d’historique, surveillance permanente, timer ou processus supplémentaire pour cette fonction.

Les données sont sauvegardées dans `state.json` avec les projets/réglages, uniquement lorsqu’elles changent ; plusieurs événements reçus ensemble partagent la même écriture. Les rendus ordinaires ne recopient pas les 2 000 entrées pour comparer les sauvegardes. Les anciennes sauvegardes restent compatibles et alimentent l’historique avec leurs projets et répertoires de panneaux. Un Mac neuf apprend les chemins locaux au fil des ouvertures et des `cd` ; aucun historique préinstallé n’est nécessaire.

Avec un shell autre que zsh, les ouvertures de projets sont enregistrées et le dossier courant est relevé à l’ouverture de la palette, sans suivi continu des `cd`. Les chemins supprimés peuvent rester dans l’historique ; leur ouverture affiche l’erreur existante, sans démarrer de shell. Les rares noms de chemin non UTF-8 ne sont pas mémorisés pour préserver la sauvegarde JSON. Les chemins d’un ancien Mac ne sont pas remappés automatiquement sur une autre machine.

Validation : tests Rust du classement, plafond, déduplication, protocole, sauvegarde/migration et clavier ; tests PTY d’un vrai zsh vierge puis d’un shell login utilisant les fichiers intégrés à Atelier. Couverture des espaces, accents, caractères de contrôle, liens symboliques, hooks existants, redirections et sous-shells. Les tests SSH simulés restent valides. Le rendu macOS n’a pas été vérifié visuellement.

## Refonte Dense + Gris mat

Design validé : barre supérieure de 40 points avec vrais boutons de fenêtre macOS, barre des projets fixe de 151 points, aucune rangée d’onglets horizontaux, panneaux continus séparés par un trait neutre. Les actions de la barre utilisent les mêmes commandes que les menus natifs (palette, ouverture, split, barre latérale, zoom). Les noms longs sont tronqués avec leur chemin en infobulle. La barre de statut permanente est supprimée ; les erreurs de sauvegarde restent visibles.

Le panneau actif garde ses couleurs. Les autres passent en niveaux de gris, mélangés à #808080 à 24 % en clair, ou #575757 à 26 % en sombre. Cette transformation est appliquée aux couleurs lors du rendu, y compris aux couleurs ANSI indexées et true-color : aucun flou, shader, rendu hors écran, timer, animation ou processus supplémentaire. Le texte DIM conserve correctement son alpha. Le curseur est affiché uniquement dans le terminal qui reçoit les entrées clavier.

La taille du texte terminal réglée par l’utilisateur est conservée. La géométrie des splits est calculée avant le rendu et le clic choisit le panneau actif avant de dessiner : une seule zone conserve ses couleurs. Les palettes de couleurs sont conservées en mémoire et empruntées au rendu, sans recréer ou cloner leurs tables ANSI à chaque panneau.

Validation : 20 tests Rust (navigation, géométrie/focus, gris mat et alpha, palette clavier, sauvegardes, intégration shell), tests PTY de SSH et du suivi de dossier ; build release et signature ad hoc. Test de démarrage et relevés de CPU/RSS avec deux shells isolés. L’intégration visuelle de la barre native macOS et la réactivité au clavier n’ont pas été mesurées visuellement ; les tests clavier existants sont sans fenêtre native. Le moteur terminal reste le même.

Relevé comparatif du 12 novembre 2026 : ancien binaire conservé avant refonte, même état temporaire (deux panneaux /bin/sh, thème sombre, fenêtre 1100×720), 12 s de stabilisation puis 6 s d’observation. Avant : RSS 120,0 Mio, 0,00 s CPU. Après : RSS 120,9 Mio, 0,01 s CPU (~0,17 % d’un cœur). Ce court test au repos ne mesure ni la latence des frappes ni le débit sous fortes sorties ; il ne garantit pas un coût nul dans toutes les situations. Script reproductible : `ATELIER_PERF_WARMUP=12 python3 tests/idle_resources.py /chemin/vers/atelier-terminal`.

## Liens web

⌘clic sur une URL `http://` ou `https://` affichée dans le terminal l’ouvre dans le navigateur par défaut. Le clic simple garde la sélection de texte. ⌘survol souligne le lien et affiche une main ; le lien est recalculé au clic pour éviter d’ouvrir une ancienne cible. La détection réutilise la regex compilée du panneau, limitée au contenu visible et aux lignes voisines pour les liens coupés par le retour visuel. Aucun index d’historique, timer, scan au repos ou nouvelle dépendance. L’ouverture utilise un thread ponctuel pour ne pas bloquer le rendu et une erreur du navigateur ne provoque plus de panique.

Les boutons d’agrandissement individuels sont retirés des en-têtes. Le bouton global en haut à droite, le menu Affichage et ⌥⌘Entrée restent disponibles.

Tests : `sh scripts/cargo.sh test --offline --locked` pour l’application et `sh scripts/cargo.sh test --manifest-path vendor/egui_term/Cargo.toml --offline --locked` pour le moteur (résolution du lien sans survol préalable, ponctuation finale, clic Command versus sélection simple).

## Audit de performances

L’audit du 12 novembre 2026 et ses limites sont documentés dans [docs/performance/2026-11-12-audit.md](performance/2026-11-12-audit.md), avec les résultats bruts et les commandes de reproduction. Le rendu recopie désormais uniquement les cellules visibles dans un vecteur réutilisable ; l’historique reste dans le moteur terminal. Les benchmarks de frappe/PTYS, grosses sorties, mémoire et fermeture sont opt-in, exclus du binaire de production.

## Docker Compose à la demande

Ouvrir ⌘⇧P → « Docker Compose… », ou Terminal → Docker Compose… dans le menu macOS. La palette permet de démarrer le projet (`up -d`), arrêter ses conteneurs (`stop`), redémarrer un service, suivre ses logs (200 dernières lignes puis suivi) ou ouvrir un shell `sh` dans un conteneur actif précis. Les réplicas sont distingués par leur nom. Aucun `down`, suppression de volume ou arrêt automatique à la fermeture du projet.

Deux badges séparent la destination du moteur et le conteneur : `LOCAL` + `DOCKER · nom`, ou `SSH · serveur` + `DOCKER · nom`. Un endpoint TCP/HTTP apparaît comme `MOTEUR · endpoint`, sans supposer qu’il est local ; une destination inconnue est indiquée explicitement. Les panneaux de logs portent `LOGS`, puisqu’ils ne sont pas un shell dans le conteneur. Après la fin de Docker, le panneau redevient un shell zsh local et retrouve le badge LOCAL. Ces badges concernent les sessions lancées depuis la palette et les `docker exec` interactifs saisis dans un shell local d’Atelier (voir ci-dessous). Les commandes lancées à l’intérieur d’une session SSH existante ne sont pas détectées. Les sessions Docker ne sont pas reconnectées automatiquement au lancement suivant.

La découverte utilise les fichiers Compose standards à la racine du projet, ou COMPOSE_FILE hérité par l’application. « Choisir les fichiers Compose… » permet des chemins personnalisés, un par ligne, relatifs au projet ou absolus ; cette sélection est sauvegardée par projet. Les fichiers `.env` et overrides sont ensuite traités par Docker Compose lui-même. Pour un `.env` qui désigne seul un fichier non standard, choisir explicitement ce fichier dans la palette. Docker et le plugin Compose doivent être installés.

Le contexte courant est lu à l’ouverture/actualisation. DOCKER_CONTEXT et DOCKER_HOST hérités par Atelier sont respectés ; les exports effectués uniquement dans un panneau ne changent pas l’environnement de l’application. Les commandes conservent le nom du contexte (ou l’hôte explicite) du snapshot pour résister à un `docker context use` concurrent. Les fichiers Compose restent locaux même lorsque le moteur est distant : il ne s’agit pas de découvrir les projets à l’intérieur d’un shell SSH.

Aucune surveillance, interrogation périodique, indexation ou dépendance Rust supplémentaire. Les interrogations CLI sont asynchrones, déclenchées uniquement à l’ouverture/actualisation de cette section ou après une action. Les statuts sont des instantanés, avec leur âge affiché. Les workers et lecteurs de sortie sont temporaires ; les requêtes ont un délai de 8 secondes, les mutations de 10 minutes, et chaque sortie capturée est plafonnée à 1 Mio. Fermer la palette annule l’attente locale ; une opération déjà envoyée au moteur peut continuer. Les logs ouverts restent un flux volontaire dans un panneau normal.

Validation : 29 tests Rust réussis (3 benchmarks manuels ignorés), incluant moteur simulé, arguments séparés, formats JSON, annulation/délai, sauvegarde des fichiers, absence de nouveaux appels au repos et retour au shell local. Test visuel dans une application isolée : menu natif, palette, démarrage simulé, shell, badges machine/conteneur, puis retour LOCAL après `exit`. La CLI Docker réelle a été vérifiée en lecture seule (version, contexte, config et ps sur un projet temporaire) ; aucun conteneur réel ni serveur distant n’a été démarré pour ces tests.

### Badge pour `docker exec` saisi au clavier

Un relais Docker privé est placé dans le PATH des nouveaux shells d’Atelier, comme le relais SSH. Il reconnaît `docker exec -it/-ti`, les options interactives longues, le nom du conteneur et les options de contexte/hôte. Il délègue les arguments sans modification au véritable Docker, puis retire le badge à la fin, en cas d’erreur ou d’interruption. Les options inconnues passent directement à Docker sans attribution de badge. Une lecture ponctuelle de la configuration locale Docker (`context inspect`) identifie le moteur ; aucune interrogation du daemon ni surveillance n’est ajoutée pour ce badge. Un échec de lecture affiche une machine non identifiée plutôt que de supposer LOCAL.

Les commandes non interactives, détachées ou redirigées ne produisent pas de badge. Les alias qui appellent `docker` via PATH bénéficient du relais ; un chemin absolu vers Docker, `sudo docker`, les `docker compose exec` manuels et les commandes dans un SSH distant contournent cette première prise en charge. Aucun fichier de configuration utilisateur n’est modifié. Le relais reste un petit processus zsh pendant l’exec interactif et disparaît ensuite ; aucun worker ne reste au repos.

Validation complémentaire : `python3 tests/docker_relay.py` utilise de vrais PTY avec un Docker simulé pour vérifier arguments, moteurs local/SSH/TCP, erreur, Ctrl-C, détachement et redirection. 31 tests Rust passent, dont installation dans PATH et protocole de badge authentifié par panneau.
