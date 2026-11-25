// How things look: amounts and months in the user's language, the words
// of the interface, and the colours of statuses and sources. Presentation
// only: what is over or under is decided in Rust.

import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

import 'l10n/app_localizations.dart';
import 'src/rust/api.dart';

/// The interface's texts in the current language: `context.words.pots`.
// An *extension* adds a getter to an existing class (BuildContext), like
// a free function called with member syntax.
extension Words on BuildContext {
  AppLocalizations get words => AppLocalizations.of(this);
}

/// An amount of cents in the current language: `1 234,56 €` in French,
/// `€1,234.56` in English.
String money(int cents, String currency) {
  // `simpleCurrency` uses the currency's symbol (€) where there is one.
  final format = NumberFormat.simpleCurrency(name: currency);
  // Dividing into a double only to display it: exact for any amount up to
  // 2^53 cents. No computation is ever done on it.
  return format.format(cents / 100);
}

/// A month (`2026-11`) as `November 2026`, or its name only (`November`).
String monthTitle(String month, {bool withYear = true}) {
  final date = DateTime.parse('$month-01');
  return (withYear ? DateFormat.yMMMM() : DateFormat.MMMM()).format(date);
}

/// A date (`2026-11-05`) as the language writes it: `5 nov. 2026`.
String dateText(String date) => DateFormat.yMMMd().format(DateTime.parse(date));

/// A time (`2026-11-24T14:05`) as the language writes it: `24 nov. 2026
/// 14:05`.
String timeText(String time) =>
    DateFormat.yMMMd().add_Hm().format(DateTime.parse(time));

/// How a line repeats, in words.
String repeatLabel(BuildContext context, Repeat repeat) => switch (repeat) {
  Repeat.monthly => context.words.repeatMonthly,
  Repeat.quarterly => context.words.repeatQuarterly,
  Repeat.yearly => context.words.repeatYearly,
  Repeat.weekly => context.words.repeatWeekly,
  Repeat.once => context.words.repeatOnce,
};

/// The colour of a cost compared with its plan (doc/scope.md): green
/// under, orange close, red over; none otherwise.
// A Dart 3 `switch` expression: like Rust's `match`, and exhaustive.
Color? statusColour(Status status) => switch (status) {
  Status.none => null,
  Status.under => Colors.green,
  Status.close => Colors.orange,
  Status.over => Colors.red,
};

/// How a month's figures were obtained, in words.
String sourceLabel(BuildContext context, Source source) => switch (source) {
  Source.actual => context.words.sourceActual,
  Source.forecast => context.words.sourceForecast,
  Source.plan => context.words.sourcePlan,
};

/// The background of a month in the calendar: actual months plain,
/// the current one highlighted, months to come faded.
Color sourceColour(BuildContext context, Source source) {
  final colours = Theme.of(context).colorScheme;
  return switch (source) {
    Source.actual => colours.surfaceContainerHigh,
    Source.forecast => colours.primaryContainer,
    Source.plan => colours.surfaceContainerLowest,
  };
}

/// An amount, coloured by `status` if it has one.
class Amount extends StatelessWidget {
  const Amount(
    this.cents,
    this.currency, {
    super.key,
    this.status = Status.none,
    this.style,
  });

  final int cents;
  final String currency;
  final Status status;
  final TextStyle? style;

  @override
  Widget build(BuildContext context) {
    final colour = statusColour(status);
    return Text(
      money(cents, currency),
      textAlign: TextAlign.end,
      // `?.`: call only if not null; `copyWith` changes one property.
      style: (style ?? const TextStyle()).copyWith(
        color: colour,
        fontWeight: colour == null ? null : FontWeight.bold,
        fontFeatures: const [FontFeature.tabularFigures()],
      ),
    );
  }
}
