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
  String get changeTransaction => 'Change the transaction';

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
  String get endIt => 'End it…';

  @override
  String get removeIt => 'Remove it';

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
  String get repeatMonthly => 'monthly';

  @override
  String get repeatQuarterly => 'quarterly';

  @override
  String get repeatYearly => 'yearly';

  @override
  String get repeatWeekly => 'weekly';

  @override
  String get repeatOnce => 'once';

  @override
  String get settings => 'Settings';

  @override
  String get yourName => 'Your name';

  @override
  String get noNameSet => 'How the other members see you';

  @override
  String get server => 'Server';

  @override
  String get noServer => 'Not on a server: your budgets stay on this device.';

  @override
  String onServer(String username, String server) {
    return '$username on $server';
  }

  @override
  String get joinServer => 'Join a server';

  @override
  String get address => 'Address';

  @override
  String get invitationToken => 'Invitation from the operator';

  @override
  String get username => 'Username';

  @override
  String get syncNow => 'Sync now';

  @override
  String synced(String sent, String received) {
    return 'Sent $sent, received $received';
  }

  @override
  String get recovery => 'Recovery';

  @override
  String get recoveryOn =>
      'A backup on the server is locked by your recovery phrase.';

  @override
  String get recoveryOff =>
      'No recovery phrase yet: if you lose all your devices, your budgets are lost.';

  @override
  String get setUpRecovery => 'Set up recovery';

  @override
  String get ownPhrase => 'Your own phrase (optional)';

  @override
  String get writePhrase =>
      'Write this phrase down and keep it safe: nobody, the server\'s operator included, can give it back.';

  @override
  String get recover => 'Recover after losing your devices';

  @override
  String get phrase => 'Recovery phrase';

  @override
  String get devices => 'Devices';

  @override
  String get showCode => 'Link another device';

  @override
  String get codeWarning =>
      'On the new device, choose “Make this device one of mine” and enter this code. Keep it secret: it opens all your budgets.';

  @override
  String get linkThis => 'Make this device one of mine';

  @override
  String get code => 'Code';

  @override
  String get copy => 'Copy';

  @override
  String get copied => 'Copied';

  @override
  String get close => 'Close';

  @override
  String get acceptInvitation => 'Join a budget with an invitation';

  @override
  String get invitation => 'Invitation';

  @override
  String get usernameIfNew => 'Username, if new on this server';

  @override
  String get askedToJoin =>
      'Asked to join: an Admin\'s device lets you in at its next sync. Then sync again.';

  @override
  String get members => 'Members';

  @override
  String get noName => '(no name yet)';

  @override
  String get you => 'you';

  @override
  String get roleAdmin => 'Admin';

  @override
  String get roleEditor => 'Editor';

  @override
  String get roleReader => 'Reader';

  @override
  String get invite => 'Invite';

  @override
  String get validDays => 'Valid for (days)';

  @override
  String get giveInvitation =>
      'Give this invitation, valid once. Then sync now and then: your device lets them in.';

  @override
  String get changeRole => 'Change the role';

  @override
  String get removeMember => 'Remove from the budget';

  @override
  String removeMemberQuestion(String name) {
    return 'Remove $name? They keep what they had, and get nothing new.';
  }

  @override
  String get leave => 'Leave the budget';

  @override
  String get leaveQuestion => 'Leave this budget?';

  @override
  String get fromOther => 'from another budget';

  @override
  String toBudget(String budget) {
    return 'Goes to the $budget budget';
  }

  @override
  String removeThereToo(String budget) {
    return 'Remove this transaction? It is removed from $budget too.';
  }

  @override
  String get amounts => 'Amounts over time';

  @override
  String get start => 'Start';

  @override
  String changeNumber(int number) {
    return 'Change $number';
  }

  @override
  String get repeats => 'Repeats';

  @override
  String get addChange => 'Add a change';

  @override
  String get changeAmount => 'Change this amount';

  @override
  String get asBefore => 'As before';

  @override
  String get removeAmount => 'Remove this change';

  @override
  String get removeAmountQuestion =>
      'Remove this change? The amount before goes on.';

  @override
  String heldIn(String budget) {
    return 'This line belongs to the $budget budget: changes are made there.';
  }

  @override
  String get whereItGoes => 'Where its money goes';

  @override
  String get redirect => 'Send it elsewhere from a date…';

  @override
  String get export => 'Export';

  @override
  String exportedTo(String folder) {
    return 'Saved in $folder';
  }
}
