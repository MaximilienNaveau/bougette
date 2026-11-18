// The small forms: names, lines, transactions, pots. They only collect
// what the user types and forward it; Rust checks it.

import 'package:flutter/material.dart';

import 'look.dart';
import 'src/rust/api.dart';

/// Runs `action`, showing its error, if any, at the bottom of the screen.
/// Returns whether it succeeded.
Future<bool> guard(
  BuildContext context,
  // `Future<void>`: any Future fits, whatever its value.
  Future<void> Function() action,
) async {
  try {
    await action();
    return true;
  } catch (error) {
    // `context.mounted`: the screen may have closed while we waited.
    if (context.mounted) {
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text('$error')));
    }
    return false;
  }
}

/// Asks for one text, starting from `initial`; null if cancelled.
Future<String?> askText(
  BuildContext context,
  String title,
  String label, {
  String initial = '',
}) {
  final controller = TextEditingController(text: initial);
  return showDialog<String>(
    context: context,
    builder: (context) => AlertDialog(
      title: Text(title),
      content: TextField(
        controller: controller,
        autofocus: true,
        decoration: InputDecoration(labelText: label),
        onSubmitted: (text) => Navigator.of(context).pop(text),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(context.words.cancel),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(controller.text),
          child: Text(context.words.ok),
        ),
      ],
    ),
  );
}

/// Asks to confirm `question`; true if confirmed.
Future<bool> confirm(BuildContext context, String question) async {
  final answer = await showDialog<bool>(
    context: context,
    builder: (context) => AlertDialog(
      content: Text(question),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(false),
          child: Text(context.words.cancel),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(true),
          child: Text(context.words.yes),
        ),
      ],
    ),
  );
  return answer ?? false;
}

/// Reads `12`, `12.5` or `12,34` as cents; null if it isn't an amount.
/// (Parsing what a person typed, not a budget rule.)
int? cents(String text) {
  final match = RegExp(r'^(\d+)(?:[.,](\d{1,2}))?$').firstMatch(text.trim());
  if (match == null) return null;
  final units = int.parse(match.group(1)!);
  final rest = (match.group(2) ?? '').padRight(2, '0');
  return units * 100 + int.parse(rest);
}

/// `12.34` for 1234 cents: what an amount field starts with.
String centsText(int cents) =>
    '${cents ~/ 100}.${(cents % 100).toString().padLeft(2, '0')}';

/// `2026-11-05` for a date.
String day(DateTime date) =>
    '${date.year}-${'${date.month}'.padLeft(2, '0')}-${'${date.day}'.padLeft(2, '0')}';

/// A button showing a date, which opens a calendar to change it.
Widget dateButton(
  BuildContext context,
  String label,
  DateTime date,
  void Function(DateTime) changed,
) {
  return TextButton.icon(
    icon: const Icon(Icons.event),
    label: Text('$label ${dateText(day(date))}'),
    onPressed: () async {
      final picked = await showDatePicker(
        context: context,
        initialDate: date,
        firstDate: DateTime(2000),
        lastDate: DateTime(2100),
      );
      if (picked != null) changed(picked);
    },
  );
}

/// A form in a dialog: `fields` rebuilt with `setState` when a choice
/// changes, and `submit` run on "OK", closing it if it succeeded. Returns
/// whether it was submitted.
Future<bool> form(
  BuildContext context,
  String title,
  List<Widget> Function(StateSetter setState) fields,
  Future<bool> Function(BuildContext context) submit,
) async {
  final done = await showDialog<bool>(
    context: context,
    // `StatefulBuilder`: a dialog with its own state (the choices made).
    builder: (context) => StatefulBuilder(
      builder: (context, setState) => AlertDialog(
        title: Text(title),
        content: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: fields(setState),
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(context.words.cancel),
          ),
          FilledButton(
            onPressed: () async {
              if (await submit(context) && context.mounted) {
                Navigator.of(context).pop(true);
              }
            },
            child: Text(context.words.ok),
          ),
        ],
      ),
    ),
  );
  return done ?? false;
}

/// A text field with a label.
Widget field(TextEditingController controller, String label) => TextField(
  controller: controller,
  decoration: InputDecoration(labelText: label),
);

/// A choice of how a line repeats.
Widget repeatChoice(
  BuildContext context,
  Repeat repeat,
  void Function(Repeat) changed,
) {
  return DropdownButton<Repeat>(
    value: repeat,
    isExpanded: true,
    items: [
      for (final option in Repeat.values)
        DropdownMenuItem(
          value: option,
          child: Text(repeatLabel(context, option)),
        ),
    ],
    onChanged: (value) => changed(value ?? repeat),
  );
}

/// The form to add a planned line of the month `view` shows. A cost's
/// money can go to one of its pots (an allocation) or to one of its
/// destinations, another budget (a contribution, written there). Returns
/// whether one was added.
Future<bool> addLine(
  BuildContext context,
  App app,
  String budget,
  MonthView view,
) {
  final name = TextEditingController();
  final category = TextEditingController();
  final amount = TextEditingController();
  var income = false;
  var repeat = Repeat.monthly;
  var first = DateTime.now();
  // Where the money goes: a pot, another budget, or neither. A record
  // `(pot, budget)` with at most one set; records compare by value, as a
  // dropdown needs.
  (PotRow?, BudgetChoice?) goesTo = (null, null);
  final goesSomewhere = view.pots.isNotEmpty || view.destinations.isNotEmpty;
  return form(
    context,
    context.words.newLine,
    (setState) => [
      field(name, context.words.name),
      field(category, context.words.category),
      field(amount, context.words.amount),
      SwitchListTile(
        title: Text(context.words.income),
        value: income,
        onChanged: goesTo == (null, null)
            ? (value) => setState(() => income = value)
            : null,
      ),
      repeatChoice(context, repeat, (value) => setState(() => repeat = value)),
      dateButton(
        context,
        context.words.firstDue,
        first,
        (date) => setState(() => first = date),
      ),
      if (goesSomewhere)
        DropdownButton<(PotRow?, BudgetChoice?)>(
          value: goesTo,
          isExpanded: true,
          items: [
            DropdownMenuItem(
              value: (null, null),
              child: Text(context.words.feedsNoPot),
            ),
            for (final pot in view.pots)
              DropdownMenuItem(
                value: (pot, null),
                child: Text(context.words.feedsPot(pot.name)),
              ),
            for (final other in view.destinations)
              DropdownMenuItem(
                value: (null, other),
                child: Text(context.words.toBudget(other.name)),
              ),
          ],
          // Money going somewhere is a cost.
          onChanged: (value) => setState(() {
            goesTo = value ?? (null, null);
            if (goesTo != (null, null)) income = false;
          }),
        ),
    ],
    (context) async {
      final value = cents(amount.text);
      if (value == null) return false;
      return guard(
        context,
        () => app.addLine(
          budget: budget,
          name: name.text,
          category: category.text,
          income: income,
          amount: value,
          repeat: repeat,
          first: day(first),
          // `.$1` and `.$2`: the record's first and second fields.
          pot: goesTo.$1?.id,
          toBudget: goesTo.$2?.id,
        ),
      );
    },
  );
}

/// The form to add a transaction, possibly realising one of the month's
/// lines, or moving money into or out of a pot. Returns whether one was
/// added.
Future<bool> addTransaction(
  BuildContext context,
  App app,
  String budget,
  MonthView view,
) {
  final amount = TextEditingController();
  final category = TextEditingController();
  final note = TextEditingController();
  var income = false;
  LineRow? line;
  // The pot, and whether the money comes from it (else goes into it).
  PotRow? pot;
  var fromPot = false;
  var date = DateTime.now();
  return form(
    context,
    context.words.newTransaction,
    (setState) => [
      DropdownButton<LineRow?>(
        value: line,
        isExpanded: true,
        items: [
          DropdownMenuItem(value: null, child: Text(context.words.unplanned)),
          for (final option in view.lines)
            DropdownMenuItem(value: option, child: Text(option.name)),
        ],
        // Choosing a line fills in its kind, category and pot.
        onChanged: (value) => setState(() {
          line = value;
          if (value != null) {
            income = value.income;
            category.text = value.category;
            pot = view.pots.where((p) => p.id == value.pot).firstOrNull;
            fromPot = false;
          }
        }),
      ),
      field(amount, context.words.amount),
      field(category, context.words.category),
      field(note, context.words.note),
      SwitchListTile(
        title: Text(context.words.income),
        value: income,
        onChanged: line == null && pot == null
            ? (value) => setState(() => income = value)
            : null,
      ),
      if (view.pots.isNotEmpty)
        DropdownButton<(PotRow, bool)?>(
          // A record `(pot, from)`: one choice per pot and direction.
          value: pot == null ? null : (pot!, fromPot),
          isExpanded: true,
          items: [
            DropdownMenuItem(value: null, child: Text(context.words.noPot)),
            for (final option in view.pots) ...[
              DropdownMenuItem(
                value: (option, false),
                child: Text(context.words.intoPot(option.name)),
              ),
              DropdownMenuItem(
                value: (option, true),
                child: Text(context.words.paidFromPot(option.name)),
              ),
            ],
          ],
          onChanged: (value) => setState(() {
            pot = value?.$1;
            fromPot = value?.$2 ?? false;
            if (value != null) income = false;
          }),
        ),
      dateButton(
        context,
        context.words.date,
        date,
        (picked) => setState(() => date = picked),
      ),
    ],
    (context) async {
      final value = cents(amount.text);
      if (value == null) return false;
      return guard(
        context,
        () => app.addTransaction(
          budget: budget,
          dateText: day(date),
          income: income,
          amount: value,
          category: category.text,
          line: line?.id,
          note: note.text,
          pot: pot?.id,
          fromPot: fromPot,
        ),
      );
    },
  );
}

/// The form to add a pot, with an optional goal. Returns whether one was
/// added.
Future<bool> addPot(BuildContext context, App app, String budget) {
  final name = TextEditingController();
  final goal = TextEditingController();
  var hasDate = false;
  var by = DateTime.now();
  return form(
    context,
    context.words.newPot,
    (setState) => [
      field(name, context.words.name),
      field(goal, context.words.goalOptional),
      SwitchListTile(
        title: Text(context.words.byADate),
        value: hasDate,
        onChanged: (value) => setState(() => hasDate = value),
      ),
      if (hasDate)
        dateButton(
          context,
          context.words.by,
          by,
          (date) => setState(() => by = date),
        ),
    ],
    (context) => guard(
      context,
      () => app.addPot(
        budget: budget,
        name: name.text,
        goal: cents(goal.text),
        by: hasDate ? day(by) : null,
      ),
    ),
  );
}
