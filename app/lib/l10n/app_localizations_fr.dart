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
  String get previousYear => 'Année précédente';

  @override
  String get nextYear => 'Année suivante';

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
  String get fromShared => 'd\'un budget commun';

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
  String get changeFromDate => 'Changer le montant à partir d\'une date…';

  @override
  String get endIt => 'Y mettre fin…';

  @override
  String endsChange(String date) {
    return 'Prend fin le $date : changer…';
  }

  @override
  String get removeIt => 'Le supprimer';

  @override
  String changeLine(String name) {
    return 'Modifier $name';
  }

  @override
  String get monthsBeforeKeep =>
      'Les mois précédents gardent l\'ancien montant.';

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
  String get changeItThere =>
      'Elle vient d\'un budget commun : modifiez-la là-bas.';

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
}
