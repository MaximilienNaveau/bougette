# Un vecteur de versions plutôt qu'un arbre de Merkle pour trouver ce qui manque

Pour savoir quels messages envoyer, deux appareils comparent leurs **vecteurs de versions** : pour chaque appareil auteur, l'horodatage de son dernier message que l'on détient. Ce qui manque à l'autre, ce sont exactement les messages de chaque auteur postérieurs à son entrée. Cela remplace l'arbre de Merkle prévu par l'[ADR 0001](0001-iroh-transport-serveur-pair.md).

C'est exact, pas une approximation, parce que :

- les messages ne changent jamais une fois écrits, et ceux d'un même auteur ont des horodatages strictement croissants (HLC suffixée par l'identifiant de l'appareil) ;
- un appareil reçoit toujours les messages d'un auteur dans l'ordre et d'un seul bloc (une transaction SQLite, tout ou rien).

Chaque appareil détient donc un **préfixe** des messages de chaque auteur, et le vecteur décrit ce préfixe sans ambiguïté. L'échange coûte une entrée par appareil, au lieu de descendre un arbre de hachages ; pas de structure à maintenir à côté du journal.

## Conséquences

- La compaction de l'historique (instantanés) devra garder, pour chaque auteur, au moins l'horodatage de son dernier message, sinon le vecteur ne décrit plus le préfixe.
- Le serveur (M2) doit voir les horodatages en clair pour calculer son vecteur : c'est déjà le cas avec un arbre de Merkle. Il ne voit ni les champs ni les valeurs, qui seront chiffrés.
- Le vecteur ne détecte pas un message altéré ou un auteur qui mentirait sur ses horodatages ; l'authenticité viendra des signatures par appareil (M2).

## Options écartées

- **Arbre de Merkle sur les horodatages** (comme Actual Budget) : utile quand on ne peut pas supposer d'ordre par auteur. Ici l'ordre existe ; l'arbre ajouterait du code et des échanges pour le même résultat.
- **Compteur par auteur** (signalé dans [l'étude Matrix](../research/matrix-as-sync.md)) : équivalent, mais il faudrait un compteur en plus de la HLC, qui joue déjà ce rôle.
