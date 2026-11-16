// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get myBudgets => 'My budgets';

  @override
  String get budgetButton => 'Budget';

  @override
  String get newBudget => 'New budget';

  @override
  String get name => 'Name';

  @override
  String get noBudget => 'No budget yet: add one.';

  @override
  String monthlyAmount(String amount) {
    return 'Monthly $amount';
  }

  @override
  String runningAmount(String amount) {
    return 'Running $amount';
  }

  @override
  String get previousYear => 'Previous year';

  @override
  String get nextYear => 'Next year';

  @override
  String get rename => 'Rename';

  @override
  String get personalBudget => 'My personal budget';

  @override
  String get sourceActual => 'actual';

  @override
  String get sourceForecast => 'forecast';

  @override
  String get sourcePlan => 'plan';

  @override
  String linesHeading(String source) {
    return 'Lines ($source)';
  }

  @override
  String get line => 'Line';

  @override
  String get due => 'Due';

  @override
  String get smoothed => 'Smoothed';

  @override
  String get actual => 'Actual';

  @override
  String get forecast => 'Forecast';

  @override
  String get totals => 'Totals';

  @override
  String get incomes => 'Incomes';

  @override
  String get costs => 'Costs';

  @override
  String get balance => 'Balance';

  @override
  String get runningAtEnd => 'Running balance at the end of the month';

  @override
  String get transactions => 'Transactions';

  @override
  String get transactionButton => 'Transaction';

  @override
  String get fromShared => 'from a shared budget';

  @override
  String get pots => 'Pots';

  @override
  String get addPot => 'Add a pot';

  @override
  String goal(String amount) {
    return 'goal $amount';
  }

  @override
  String goalBy(String amount, String date) {
    return 'goal $amount by $date';
  }

  @override
  String until(String date) {
    return 'until $date';
  }

  @override
  String get addLine => 'Add a line';

  @override
  String get cancel => 'Cancel';

  @override
  String get ok => 'OK';

  @override
  String get yes => 'Yes';

  @override
  String get category => 'Category';

  @override
  String get amount => 'Amount';

  @override
  String get income => 'Income';

  @override
  String get note => 'Note';

  @override
  String get date => 'Date';

  @override
  String get firstDue => 'First due';

  @override
  String get from => 'From';

  @override
  String get lastDay => 'Last day';

  @override
  String get by => 'By';

  @override
  String get newLine => 'New line';

  @override
  String get newTransaction => 'New transaction';

  @override
  String get newPot => 'New pot';

  @override
  String get feedsNoPot => 'Feeds no pot';

  @override
  String feedsPot(String pot) {
    return 'Feeds $pot';
  }

  @override
  String get unplanned => 'Unplanned';

  @override
  String get noPot => 'No pot';

  @override
  String intoPot(String pot) {
    return 'Into $pot';
  }

  @override
  String paidFromPot(String pot) {
    return 'Paid from $pot';
  }

  @override
  String get goalOptional => 'Goal (optional)';

  @override
  String get byADate => 'By a date';

  @override
  String get changeFromDate => 'Change the amount from a date…';

  @override
  String get endIt => 'End it…';

  @override
  String endsChange(String date) {
    return 'Ends $date: change…';
  }

  @override
  String get removeIt => 'Remove it';

  @override
  String changeLine(String name) {
    return 'Change $name';
  }

  @override
  String get monthsBeforeKeep => 'The months before keep the old amount.';

  @override
  String endLine(String name) {
    return 'End $name';
  }

  @override
  String get goesOn => 'Goes on';

  @override
  String removeQuestion(String name) {
    return 'Remove $name?';
  }

  @override
  String get removeTransactionQuestion => 'Remove this transaction?';

  @override
  String get changeItThere => 'It comes from a shared budget: change it there.';

  @override
  String get repeatMonthly => 'monthly';

  @override
  String get repeatQuarterly => 'quarterly';

  @override
  String get repeatYearly => 'yearly';

  @override
  String get repeatWeekly => 'weekly';

  @override
  String get repeatOnce => 'once';
}
