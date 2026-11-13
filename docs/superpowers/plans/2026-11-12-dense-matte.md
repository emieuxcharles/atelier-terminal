# Dense + Gris mat

Design approuvé : maquette Dense, sans onglets horizontaux, barre 40px, sidebar 151px, terminaux continus. Panneau actif intact ; panneaux inactifs désaturés puis voile gris (#808080 24% clair, #575757 26% sombre). Aucun cadre vert.

1. Relever release existante (fenêtre isolée, deux shells, CPU au repos et RSS).
2. Tests des couleurs (actif intact, gris neutre, deux thèmes) et de la géométrie/focus avant dessin.
3. Ajouter chrome compact avec vrais boutons macOS, actions utilisant execute_action, sidebar fixe et lignes tronquées. Palette/réglages gardent leur logique.
4. Transformer les couleurs pendant le rendu du terminal, y compris ANSI true-color ; pas de shader, filtre, timer ou dépendance. Mettre en cache les thèmes et éviter le clonage de leur palette par panneau.
5. Tests, build et comparaison ressources identique. Documenter limites de validation visuelle native.

Terminé : UI, thèmes, rendu neutre, cache/emprunt des palettes, tests et bundle signé. Relecture ciblée : correction du traitement alpha des cellules DIM, test de régression ajouté. 20 tests et scripts PTY passent. Comparaison repos stabilisée : 120,0 → 120,9 Mio ; 0,00 → 0,01 s CPU sur 6 s. Validation visuelle native non effectuée ; aucune mesure de latence de frappe annoncée.
