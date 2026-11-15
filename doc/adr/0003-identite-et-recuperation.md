# Identité et récupération : trois couches séparées, une phrase de récupération

Il n'y a pas de compte avec mot de passe pour l'usage quotidien. La sécurité repose sur trois couches distinctes :

1. **Accès au serveur** : un lien d'invitation et un nom d'utilisateur propre au serveur. L'opérateur du serveur peut toujours le rétablir, mais cet accès **ne donne jamais accès au contenu**.
2. **Identité du membre** : une clé, partagée entre ses appareils par QR code d'un appareil à l'autre.
3. **Clés des budgets** : détenues uniquement par les appareils des membres de chaque budget.

Pour la récupération, le membre dépose sur le serveur une **sauvegarde chiffrée** (clé de membre et clés de ses budgets), verrouillée par une **phrase de récupération**. Par défaut, elle est générée par l'app ; le membre peut choisir sa propre phrase, mais les phrases faibles sont refusées. C'est le même principe que la clé ou phrase de sécurité de Matrix/Element.

Si un appareil arrivait à récupérer le contenu avec seulement un lien et un nom d'utilisateur, l'opérateur du serveur pourrait se faire passer pour n'importe quel membre : c'est la raison de cette séparation.

## Conséquences

- L'opérateur, ou quelqu'un qui volerait la base du serveur, peut tenter de **deviner** la phrase hors ligne. D'où Argon2id, une phrase générée par défaut et le refus des phrases faibles.
- Tout perdu, phrase comprise :
  - les budgets partagés reviennent seulement après qu'un membre a vérifié l'identité de la personne (QR en personne), **jamais automatiquement** ;
  - le budget individuel est perdu.
- Plus tard : déposer, au choix du membre, une part de son secret chez une personne de confiance (récupération sociale).
