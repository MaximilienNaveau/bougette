// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for French (`fr`).
class AppLocalizationsFr extends AppLocalizations {
  AppLocalizationsFr([String locale = 'fr']) : super(locale);

  @override
  String get myBudgets => 'Mes budgets';

  @override
  String get budgetButton => 'Budget';

  @override
  String get newBudget => 'Nouveau budget';

  @override
  String get name => 'Nom';

  @override
  String get noBudget => 'Pas encore de budget : ajoutez-en un.';

  @override
  String monthlyAmount(String amount) {
    return 'Mensuel $amount';
  }

  @override
  String runningAmount(String amount) {
    return 'Cumulé $amount';
  }

  @override
  String get rename => 'Renommer';

  @override
  String get personalBudget => 'Mon budget personnel';

  @override
  String get sourceActual => 'réel';

  @override
  String get sourceForecast => 'projection';

  @override
  String get sourcePlan => 'prévu';

  @override
  String linesHeading(String source) {
    return 'Postes ($source)';
  }

  @override
  String get line => 'Poste';

  @override
  String get due => 'Dû';

  @override
  String get smoothed => 'Lissé';

  @override
  String get actual => 'Réel';

  @override
  String get forecast => 'Projection';

  @override
  String get totals => 'Totaux';

  @override
  String get incomes => 'Revenus';

  @override
  String get costs => 'Dépenses';

  @override
  String get balance => 'Solde';

  @override
  String get runningAtEnd => 'Solde cumulé à la fin du mois';

  @override
  String get transactions => 'Opérations';

  @override
  String get transactionButton => 'Opération';

  @override
  String get pots => 'Cagnottes';

  @override
  String get addPot => 'Ajouter une cagnotte';

  @override
  String goal(String amount) {
    return 'objectif $amount';
  }

  @override
  String goalBy(String amount, String date) {
    return 'objectif $amount d\'ici le $date';
  }

  @override
  String until(String date) {
    return 'jusqu\'au $date';
  }

  @override
  String get addLine => 'Ajouter un poste';

  @override
  String get cancel => 'Annuler';

  @override
  String get ok => 'OK';

  @override
  String get yes => 'Oui';

  @override
  String get category => 'Catégorie';

  @override
  String get amount => 'Montant';

  @override
  String get income => 'Revenu';

  @override
  String get note => 'Note';

  @override
  String get date => 'Date';

  @override
  String get firstDue => 'Première échéance';

  @override
  String get from => 'À partir du';

  @override
  String get lastDay => 'Dernier jour';

  @override
  String get by => 'D\'ici le';

  @override
  String get newLine => 'Nouveau poste';

  @override
  String get newTransaction => 'Nouvelle opération';

  @override
  String get changeTransaction => 'Modifier l\'opération';

  @override
  String get newPot => 'Nouvelle cagnotte';

  @override
  String get feedsNoPot => 'N\'alimente aucune cagnotte';

  @override
  String feedsPot(String pot) {
    return 'Alimente $pot';
  }

  @override
  String get unplanned => 'Imprévu';

  @override
  String get noPot => 'Aucune cagnotte';

  @override
  String intoPot(String pot) {
    return 'Vers $pot';
  }

  @override
  String paidFromPot(String pot) {
    return 'Payé par $pot';
  }

  @override
  String get goalOptional => 'Objectif (facultatif)';

  @override
  String get byADate => 'Avec une date';

  @override
  String get endIt => 'Y mettre fin…';

  @override
  String get removeIt => 'Le supprimer';

  @override
  String endLine(String name) {
    return 'Fin de $name';
  }

  @override
  String get goesOn => 'Continue';

  @override
  String removeQuestion(String name) {
    return 'Supprimer $name ?';
  }

  @override
  String get removeTransactionQuestion => 'Supprimer cette opération ?';

  @override
  String get repeatMonthly => 'mensuel';

  @override
  String get repeatQuarterly => 'trimestriel';

  @override
  String get repeatYearly => 'annuel';

  @override
  String get repeatWeekly => 'hebdomadaire';

  @override
  String get repeatOnce => 'une fois';

  @override
  String get settings => 'Réglages';

  @override
  String get yourName => 'Votre nom';

  @override
  String get noNameSet => 'Comment les autres membres vous voient';

  @override
  String get server => 'Serveur';

  @override
  String get noServer =>
      'Sur aucun serveur : vos budgets restent sur cet appareil.';

  @override
  String onServer(String username, String server) {
    return '$username sur $server';
  }

  @override
  String get joinServer => 'Rejoindre un serveur';

  @override
  String get address => 'Adresse';

  @override
  String get invitationToken => 'Invitation de l\'opérateur';

  @override
  String get username => 'Nom d\'utilisateur';

  @override
  String get syncNow => 'Synchroniser';

  @override
  String synced(String sent, String received) {
    return 'Envoyé $sent, reçu $received';
  }

  @override
  String get recovery => 'Récupération';

  @override
  String get recoveryOn =>
      'Une sauvegarde sur le serveur est verrouillée par votre phrase de récupération.';

  @override
  String get recoveryOff =>
      'Pas encore de phrase de récupération : si vous perdez tous vos appareils, vos budgets sont perdus.';

  @override
  String get setUpRecovery => 'Mettre en place la récupération';

  @override
  String get ownPhrase => 'Votre propre phrase (facultatif)';

  @override
  String get writePhrase =>
      'Notez cette phrase et gardez-la en lieu sûr : personne, pas même l\'opérateur du serveur, ne peut vous la redonner.';

  @override
  String get recover => 'Récupérer après la perte de vos appareils';

  @override
  String get phrase => 'Phrase de récupération';

  @override
  String get devices => 'Appareils';

  @override
  String get showCode => 'Lier un autre appareil';

  @override
  String get codeWarning =>
      'Sur le nouvel appareil, choisissez « Faire de cet appareil l\'un des miens » et saisissez ce code. Gardez-le secret : il ouvre tous vos budgets.';

  @override
  String get linkThis => 'Faire de cet appareil l\'un des miens';

  @override
  String get code => 'Code';

  @override
  String get copy => 'Copier';

  @override
  String get copied => 'Copié';

  @override
  String get close => 'Fermer';

  @override
  String get acceptInvitation => 'Rejoindre un budget avec une invitation';

  @override
  String get invitation => 'Invitation';

  @override
  String get usernameIfNew => 'Nom d\'utilisateur, si nouveau sur ce serveur';

  @override
  String get askedToJoin =>
      'Demande envoyée : l\'appareil d\'un Admin vous fera entrer à sa prochaine synchronisation. Synchronisez ensuite.';

  @override
  String get members => 'Membres';

  @override
  String get noName => '(pas encore de nom)';

  @override
  String get you => 'vous';

  @override
  String get roleAdmin => 'Admin';

  @override
  String get roleEditor => 'Éditeur';

  @override
  String get roleReader => 'Lecteur';

  @override
  String get invite => 'Inviter';

  @override
  String get validDays => 'Valable (jours)';

  @override
  String get giveInvitation =>
      'Donnez cette invitation, valable une fois. Synchronisez ensuite de temps en temps : votre appareil fera entrer la personne.';

  @override
  String get changeRole => 'Changer le rôle';

  @override
  String get removeMember => 'Retirer du budget';

  @override
  String removeMemberQuestion(String name) {
    return 'Retirer $name ? La personne garde ce qu\'elle avait, et ne reçoit plus rien.';
  }

  @override
  String get leave => 'Quitter le budget';

  @override
  String get leaveQuestion => 'Quitter ce budget ?';

  @override
  String get fromOther => 'd\'un autre budget';

  @override
  String toBudget(String budget) {
    return 'Versé au budget $budget';
  }

  @override
  String removeThereToo(String budget) {
    return 'Supprimer cette opération ? Elle sera aussi supprimée de $budget.';
  }

  @override
  String get amounts => 'Montants dans le temps';

  @override
  String get start => 'Début';

  @override
  String changeNumber(int number) {
    return 'Changement $number';
  }

  @override
  String get repeats => 'Échéance';

  @override
  String get addChange => 'Ajouter un changement';

  @override
  String get changeAmount => 'Modifier ce montant';

  @override
  String get asBefore => 'Comme avant';

  @override
  String get removeAmount => 'Supprimer ce changement';

  @override
  String get removeAmountQuestion =>
      'Supprimer ce changement ? Le montant précédent continue.';

  @override
  String heldIn(String budget) {
    return 'Ce poste appartient au budget $budget : les modifications y sont faites.';
  }

  @override
  String get whereItGoes => 'Où va son argent';

  @override
  String get redirect => 'L\'envoyer ailleurs à partir d\'une date…';

  @override
  String get export => 'Exporter';

  @override
  String exportedTo(String folder) {
    return 'Enregistré dans $folder';
  }

  @override
  String get removed => 'Supprimé';

  @override
  String get undo => 'Annuler';

  @override
  String get trash => 'Corbeille';

  @override
  String get trashEmpty => 'Rien de supprimé ces 30 derniers jours.';

  @override
  String get restore => 'Restaurer';

  @override
  String get restored => 'Restauré';

  @override
  String deletedOn(String date) {
    return 'Supprimé le $date';
  }

  @override
  String get emptyTrash => 'Vider la corbeille';

  @override
  String get emptyTrashQuestion =>
      'Vider la corbeille ? Ce qu\'elle contient ne pourra plus être restauré.';
}
