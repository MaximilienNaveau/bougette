import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_en.dart';
import 'app_localizations_fr.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'l10n/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations)!;
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('en'),
    Locale('fr'),
  ];

  /// No description provided for @myBudgets.
  ///
  /// In en, this message translates to:
  /// **'My budgets'**
  String get myBudgets;

  /// No description provided for @budgetButton.
  ///
  /// In en, this message translates to:
  /// **'Budget'**
  String get budgetButton;

  /// No description provided for @newBudget.
  ///
  /// In en, this message translates to:
  /// **'New budget'**
  String get newBudget;

  /// No description provided for @name.
  ///
  /// In en, this message translates to:
  /// **'Name'**
  String get name;

  /// No description provided for @noBudget.
  ///
  /// In en, this message translates to:
  /// **'No budget yet: add one.'**
  String get noBudget;

  /// No description provided for @monthlyAmount.
  ///
  /// In en, this message translates to:
  /// **'Monthly {amount}'**
  String monthlyAmount(String amount);

  /// No description provided for @runningAmount.
  ///
  /// In en, this message translates to:
  /// **'Running {amount}'**
  String runningAmount(String amount);

  /// No description provided for @previousYear.
  ///
  /// In en, this message translates to:
  /// **'Previous year'**
  String get previousYear;

  /// No description provided for @nextYear.
  ///
  /// In en, this message translates to:
  /// **'Next year'**
  String get nextYear;

  /// No description provided for @rename.
  ///
  /// In en, this message translates to:
  /// **'Rename'**
  String get rename;

  /// No description provided for @personalBudget.
  ///
  /// In en, this message translates to:
  /// **'My personal budget'**
  String get personalBudget;

  /// No description provided for @sourceActual.
  ///
  /// In en, this message translates to:
  /// **'actual'**
  String get sourceActual;

  /// No description provided for @sourceForecast.
  ///
  /// In en, this message translates to:
  /// **'forecast'**
  String get sourceForecast;

  /// No description provided for @sourcePlan.
  ///
  /// In en, this message translates to:
  /// **'plan'**
  String get sourcePlan;

  /// No description provided for @linesHeading.
  ///
  /// In en, this message translates to:
  /// **'Lines ({source})'**
  String linesHeading(String source);

  /// No description provided for @line.
  ///
  /// In en, this message translates to:
  /// **'Line'**
  String get line;

  /// No description provided for @due.
  ///
  /// In en, this message translates to:
  /// **'Due'**
  String get due;

  /// No description provided for @smoothed.
  ///
  /// In en, this message translates to:
  /// **'Smoothed'**
  String get smoothed;

  /// No description provided for @actual.
  ///
  /// In en, this message translates to:
  /// **'Actual'**
  String get actual;

  /// No description provided for @forecast.
  ///
  /// In en, this message translates to:
  /// **'Forecast'**
  String get forecast;

  /// No description provided for @totals.
  ///
  /// In en, this message translates to:
  /// **'Totals'**
  String get totals;

  /// No description provided for @incomes.
  ///
  /// In en, this message translates to:
  /// **'Incomes'**
  String get incomes;

  /// No description provided for @costs.
  ///
  /// In en, this message translates to:
  /// **'Costs'**
  String get costs;

  /// No description provided for @balance.
  ///
  /// In en, this message translates to:
  /// **'Balance'**
  String get balance;

  /// No description provided for @runningAtEnd.
  ///
  /// In en, this message translates to:
  /// **'Running balance at the end of the month'**
  String get runningAtEnd;

  /// No description provided for @transactions.
  ///
  /// In en, this message translates to:
  /// **'Transactions'**
  String get transactions;

  /// No description provided for @transactionButton.
  ///
  /// In en, this message translates to:
  /// **'Transaction'**
  String get transactionButton;

  /// No description provided for @fromShared.
  ///
  /// In en, this message translates to:
  /// **'from a shared budget'**
  String get fromShared;

  /// No description provided for @pots.
  ///
  /// In en, this message translates to:
  /// **'Pots'**
  String get pots;

  /// No description provided for @addPot.
  ///
  /// In en, this message translates to:
  /// **'Add a pot'**
  String get addPot;

  /// No description provided for @goal.
  ///
  /// In en, this message translates to:
  /// **'goal {amount}'**
  String goal(String amount);

  /// No description provided for @goalBy.
  ///
  /// In en, this message translates to:
  /// **'goal {amount} by {date}'**
  String goalBy(String amount, String date);

  /// No description provided for @until.
  ///
  /// In en, this message translates to:
  /// **'until {date}'**
  String until(String date);

  /// No description provided for @addLine.
  ///
  /// In en, this message translates to:
  /// **'Add a line'**
  String get addLine;

  /// No description provided for @cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// No description provided for @ok.
  ///
  /// In en, this message translates to:
  /// **'OK'**
  String get ok;

  /// No description provided for @yes.
  ///
  /// In en, this message translates to:
  /// **'Yes'**
  String get yes;

  /// No description provided for @category.
  ///
  /// In en, this message translates to:
  /// **'Category'**
  String get category;

  /// No description provided for @amount.
  ///
  /// In en, this message translates to:
  /// **'Amount'**
  String get amount;

  /// No description provided for @income.
  ///
  /// In en, this message translates to:
  /// **'Income'**
  String get income;

  /// No description provided for @note.
  ///
  /// In en, this message translates to:
  /// **'Note'**
  String get note;

  /// No description provided for @date.
  ///
  /// In en, this message translates to:
  /// **'Date'**
  String get date;

  /// No description provided for @firstDue.
  ///
  /// In en, this message translates to:
  /// **'First due'**
  String get firstDue;

  /// No description provided for @from.
  ///
  /// In en, this message translates to:
  /// **'From'**
  String get from;

  /// No description provided for @lastDay.
  ///
  /// In en, this message translates to:
  /// **'Last day'**
  String get lastDay;

  /// No description provided for @by.
  ///
  /// In en, this message translates to:
  /// **'By'**
  String get by;

  /// No description provided for @newLine.
  ///
  /// In en, this message translates to:
  /// **'New line'**
  String get newLine;

  /// No description provided for @newTransaction.
  ///
  /// In en, this message translates to:
  /// **'New transaction'**
  String get newTransaction;

  /// No description provided for @newPot.
  ///
  /// In en, this message translates to:
  /// **'New pot'**
  String get newPot;

  /// No description provided for @feedsNoPot.
  ///
  /// In en, this message translates to:
  /// **'Feeds no pot'**
  String get feedsNoPot;

  /// No description provided for @feedsPot.
  ///
  /// In en, this message translates to:
  /// **'Feeds {pot}'**
  String feedsPot(String pot);

  /// No description provided for @unplanned.
  ///
  /// In en, this message translates to:
  /// **'Unplanned'**
  String get unplanned;

  /// No description provided for @noPot.
  ///
  /// In en, this message translates to:
  /// **'No pot'**
  String get noPot;

  /// No description provided for @intoPot.
  ///
  /// In en, this message translates to:
  /// **'Into {pot}'**
  String intoPot(String pot);

  /// No description provided for @paidFromPot.
  ///
  /// In en, this message translates to:
  /// **'Paid from {pot}'**
  String paidFromPot(String pot);

  /// No description provided for @goalOptional.
  ///
  /// In en, this message translates to:
  /// **'Goal (optional)'**
  String get goalOptional;

  /// No description provided for @byADate.
  ///
  /// In en, this message translates to:
  /// **'By a date'**
  String get byADate;

  /// No description provided for @changeFromDate.
  ///
  /// In en, this message translates to:
  /// **'Change the amount from a date…'**
  String get changeFromDate;

  /// No description provided for @endIt.
  ///
  /// In en, this message translates to:
  /// **'End it…'**
  String get endIt;

  /// No description provided for @endsChange.
  ///
  /// In en, this message translates to:
  /// **'Ends {date}: change…'**
  String endsChange(String date);

  /// No description provided for @removeIt.
  ///
  /// In en, this message translates to:
  /// **'Remove it'**
  String get removeIt;

  /// No description provided for @changeLine.
  ///
  /// In en, this message translates to:
  /// **'Change {name}'**
  String changeLine(String name);

  /// No description provided for @monthsBeforeKeep.
  ///
  /// In en, this message translates to:
  /// **'The months before keep the old amount.'**
  String get monthsBeforeKeep;

  /// No description provided for @endLine.
  ///
  /// In en, this message translates to:
  /// **'End {name}'**
  String endLine(String name);

  /// No description provided for @goesOn.
  ///
  /// In en, this message translates to:
  /// **'Goes on'**
  String get goesOn;

  /// No description provided for @removeQuestion.
  ///
  /// In en, this message translates to:
  /// **'Remove {name}?'**
  String removeQuestion(String name);

  /// No description provided for @removeTransactionQuestion.
  ///
  /// In en, this message translates to:
  /// **'Remove this transaction?'**
  String get removeTransactionQuestion;

  /// No description provided for @changeItThere.
  ///
  /// In en, this message translates to:
  /// **'It comes from a shared budget: change it there.'**
  String get changeItThere;

  /// No description provided for @repeatMonthly.
  ///
  /// In en, this message translates to:
  /// **'monthly'**
  String get repeatMonthly;

  /// No description provided for @repeatQuarterly.
  ///
  /// In en, this message translates to:
  /// **'quarterly'**
  String get repeatQuarterly;

  /// No description provided for @repeatYearly.
  ///
  /// In en, this message translates to:
  /// **'yearly'**
  String get repeatYearly;

  /// No description provided for @repeatWeekly.
  ///
  /// In en, this message translates to:
  /// **'weekly'**
  String get repeatWeekly;

  /// No description provided for @repeatOnce.
  ///
  /// In en, this message translates to:
  /// **'once'**
  String get repeatOnce;

  /// No description provided for @settings.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get settings;

  /// No description provided for @yourName.
  ///
  /// In en, this message translates to:
  /// **'Your name'**
  String get yourName;

  /// No description provided for @noNameSet.
  ///
  /// In en, this message translates to:
  /// **'How the other members see you'**
  String get noNameSet;

  /// No description provided for @server.
  ///
  /// In en, this message translates to:
  /// **'Server'**
  String get server;

  /// No description provided for @noServer.
  ///
  /// In en, this message translates to:
  /// **'Not on a server: your budgets stay on this device.'**
  String get noServer;

  /// No description provided for @onServer.
  ///
  /// In en, this message translates to:
  /// **'{username} on {server}'**
  String onServer(String username, String server);

  /// No description provided for @joinServer.
  ///
  /// In en, this message translates to:
  /// **'Join a server'**
  String get joinServer;

  /// No description provided for @address.
  ///
  /// In en, this message translates to:
  /// **'Address'**
  String get address;

  /// No description provided for @invitationToken.
  ///
  /// In en, this message translates to:
  /// **'Invitation from the operator'**
  String get invitationToken;

  /// No description provided for @username.
  ///
  /// In en, this message translates to:
  /// **'Username'**
  String get username;

  /// No description provided for @syncNow.
  ///
  /// In en, this message translates to:
  /// **'Sync now'**
  String get syncNow;

  /// No description provided for @synced.
  ///
  /// In en, this message translates to:
  /// **'Sent {sent}, received {received}'**
  String synced(String sent, String received);

  /// No description provided for @recovery.
  ///
  /// In en, this message translates to:
  /// **'Recovery'**
  String get recovery;

  /// No description provided for @recoveryOn.
  ///
  /// In en, this message translates to:
  /// **'A backup on the server is locked by your recovery phrase.'**
  String get recoveryOn;

  /// No description provided for @recoveryOff.
  ///
  /// In en, this message translates to:
  /// **'No recovery phrase yet: if you lose all your devices, your budgets are lost.'**
  String get recoveryOff;

  /// No description provided for @setUpRecovery.
  ///
  /// In en, this message translates to:
  /// **'Set up recovery'**
  String get setUpRecovery;

  /// No description provided for @ownPhrase.
  ///
  /// In en, this message translates to:
  /// **'Your own phrase (optional)'**
  String get ownPhrase;

  /// No description provided for @writePhrase.
  ///
  /// In en, this message translates to:
  /// **'Write this phrase down and keep it safe: nobody, the server\'s operator included, can give it back.'**
  String get writePhrase;

  /// No description provided for @recover.
  ///
  /// In en, this message translates to:
  /// **'Recover after losing your devices'**
  String get recover;

  /// No description provided for @phrase.
  ///
  /// In en, this message translates to:
  /// **'Recovery phrase'**
  String get phrase;

  /// No description provided for @devices.
  ///
  /// In en, this message translates to:
  /// **'Devices'**
  String get devices;

  /// No description provided for @showCode.
  ///
  /// In en, this message translates to:
  /// **'Link another device'**
  String get showCode;

  /// No description provided for @codeWarning.
  ///
  /// In en, this message translates to:
  /// **'On the new device, choose “Make this device one of mine” and enter this code. Keep it secret: it opens all your budgets.'**
  String get codeWarning;

  /// No description provided for @linkThis.
  ///
  /// In en, this message translates to:
  /// **'Make this device one of mine'**
  String get linkThis;

  /// No description provided for @code.
  ///
  /// In en, this message translates to:
  /// **'Code'**
  String get code;

  /// No description provided for @copy.
  ///
  /// In en, this message translates to:
  /// **'Copy'**
  String get copy;

  /// No description provided for @copied.
  ///
  /// In en, this message translates to:
  /// **'Copied'**
  String get copied;

  /// No description provided for @close.
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get close;

  /// No description provided for @acceptInvitation.
  ///
  /// In en, this message translates to:
  /// **'Join a budget with an invitation'**
  String get acceptInvitation;

  /// No description provided for @invitation.
  ///
  /// In en, this message translates to:
  /// **'Invitation'**
  String get invitation;

  /// No description provided for @usernameIfNew.
  ///
  /// In en, this message translates to:
  /// **'Username, if new on this server'**
  String get usernameIfNew;

  /// No description provided for @askedToJoin.
  ///
  /// In en, this message translates to:
  /// **'Asked to join: an Admin\'s device lets you in at its next sync. Then sync again.'**
  String get askedToJoin;

  /// No description provided for @members.
  ///
  /// In en, this message translates to:
  /// **'Members'**
  String get members;

  /// No description provided for @noName.
  ///
  /// In en, this message translates to:
  /// **'(no name yet)'**
  String get noName;

  /// No description provided for @you.
  ///
  /// In en, this message translates to:
  /// **'you'**
  String get you;

  /// No description provided for @roleAdmin.
  ///
  /// In en, this message translates to:
  /// **'Admin'**
  String get roleAdmin;

  /// No description provided for @roleEditor.
  ///
  /// In en, this message translates to:
  /// **'Editor'**
  String get roleEditor;

  /// No description provided for @roleReader.
  ///
  /// In en, this message translates to:
  /// **'Reader'**
  String get roleReader;

  /// No description provided for @invite.
  ///
  /// In en, this message translates to:
  /// **'Invite'**
  String get invite;

  /// No description provided for @validDays.
  ///
  /// In en, this message translates to:
  /// **'Valid for (days)'**
  String get validDays;

  /// No description provided for @giveInvitation.
  ///
  /// In en, this message translates to:
  /// **'Give this invitation, valid once. Then sync now and then: your device lets them in.'**
  String get giveInvitation;

  /// No description provided for @changeRole.
  ///
  /// In en, this message translates to:
  /// **'Change the role'**
  String get changeRole;

  /// No description provided for @removeMember.
  ///
  /// In en, this message translates to:
  /// **'Remove from the budget'**
  String get removeMember;

  /// No description provided for @removeMemberQuestion.
  ///
  /// In en, this message translates to:
  /// **'Remove {name}? They keep what they had, and get nothing new.'**
  String removeMemberQuestion(String name);

  /// No description provided for @leave.
  ///
  /// In en, this message translates to:
  /// **'Leave the budget'**
  String get leave;

  /// No description provided for @leaveQuestion.
  ///
  /// In en, this message translates to:
  /// **'Leave this budget?'**
  String get leaveQuestion;
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['en', 'fr'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'en':
      return AppLocalizationsEn();
    case 'fr':
      return AppLocalizationsFr();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
