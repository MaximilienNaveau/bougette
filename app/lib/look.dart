// How figures look: amounts, and the colours of statuses and sources.
// Presentation only: what is over or under is decided in Rust.

import 'package:flutter/material.dart';

import 'src/rust/api.dart';

/// An amount of cents as `-1 234.56 EUR`. (Proper localisation, with
/// decimal commas and currency symbols, comes with the translations.)
String money(int cents, String currency) {
  final sign = cents < 0 ? '-' : '';
  final units = (cents.abs() ~/ 100).toString(); // `~/`: integer division
  final rest = (cents.abs() % 100).toString().padLeft(2, '0');
  // Groups of three digits, from the right, separated by spaces.
  final grouped = StringBuffer();
  for (var i = 0; i < units.length; i++) {
    if (i > 0 && (units.length - i) % 3 == 0) grouped.write(' ');
    grouped.write(units[i]);
  }
  return '$sign$grouped.$rest $currency';
}

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
String sourceLabel(Source source) => switch (source) {
  Source.actual => 'actual',
  Source.forecast => 'forecast',
  Source.plan => 'plan',
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
