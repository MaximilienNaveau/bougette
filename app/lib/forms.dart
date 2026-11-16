// The small forms: names, lines, transactions, pots. They only collect
// what the user types and forward it; Rust checks it.

import 'package:flutter/material.dart';

import 'src/rust/api.dart';

/// Runs `action`, showing its error, if any, at the bottom of the screen.
/// Returns whether it succeeded.
Future<bool> guard(
  BuildContext context,
  Future<Object?> Function() action,
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
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(controller.text),
          child: const Text('OK'),
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
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(true),
          child: const Text('Yes'),
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
    label: Text('$label ${day(date)}'),
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
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () async {
              if (await submit(context) && context.mounted) {
                Navigator.of(context).pop(true);
              }
            },
            child: const Text('OK'),
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
Widget repeatChoice(Repeat repeat, void Function(Repeat) changed) {
  return DropdownButton<Repeat>(
    value: repeat,
    isExpanded: true,
    items: [
      for (final option in Repeat.values)
        DropdownMenuItem(value: option, child: Text(option.name)),
    ],
    onChanged: (value) => changed(value ?? repeat),
  );
}

/// The form to add a planned line, possibly an allocation to one of
/// `pots`. Returns whether one was added.
Future<bool> addLine(
  BuildContext context,
  App app,
  String budget,
  List<PotRow> pots,
) {
  final name = TextEditingController();
  final category = TextEditingController();
  final amount = TextEditingController();
  var income = false;
  var repeat = Repeat.monthly;
  var first = DateTime.now();
  PotRow? pot;
  return form(
    context,
    'New line',
    (setState) => [
      field(name, 'Name'),
      field(category, 'Category'),
      field(amount, 'Amount'),
      SwitchListTile(
        title: const Text('Income'),
        value: income,
        onChanged: pot == null
            ? (value) => setState(() => income = value)
            : null,
      ),
      repeatChoice(repeat, (value) => setState(() => repeat = value)),
      dateButton(
        context,
        'First due',
        first,
        (date) => setState(() => first = date),
      ),
      if (pots.isNotEmpty)
        DropdownButton<PotRow?>(
          value: pot,
          isExpanded: true,
          items: [
            const DropdownMenuItem(value: null, child: Text('Feeds no pot')),
            for (final option in pots)
              DropdownMenuItem(
                value: option,
                child: Text('Feeds ${option.name}'),
              ),
          ],
          // An allocation is a cost.
          onChanged: (value) => setState(() {
            pot = value;
            if (value != null) income = false;
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
          pot: pot?.id,
        ),
      );
    },
  );
}

/// What can be done to a line: change it from a date, end it, remove it.
/// Returns whether something changed.
Future<bool> editLine(
  BuildContext context,
  App app,
  String budget,
  LineRow line,
) async {
  final choice = await showDialog<String>(
    context: context,
    builder: (context) => SimpleDialog(
      title: Text(line.name),
      children: [
        SimpleDialogOption(
          onPressed: () => Navigator.of(context).pop('change'),
          child: const Text('Change the amount from a date…'),
        ),
        SimpleDialogOption(
          onPressed: () => Navigator.of(context).pop('end'),
          child: Text(
            line.endsOn == null ? 'End it…' : 'Ends ${line.endsOn}: change…',
          ),
        ),
        SimpleDialogOption(
          onPressed: () => Navigator.of(context).pop('remove'),
          child: const Text('Remove it'),
        ),
      ],
    ),
  );
  if (choice == null || !context.mounted) return false;
  switch (choice) {
    case 'change':
      final amount = TextEditingController(text: centsText(line.amount));
      var repeat = line.repeat ?? Repeat.monthly;
      var from = DateTime.now();
      return form(
        context,
        'Change ${line.name}',
        (setState) => [
          field(amount, 'Amount'),
          repeatChoice(repeat, (value) => setState(() => repeat = value)),
          dateButton(
            context,
            'From',
            from,
            (date) => setState(() => from = date),
          ),
          const Text('The months before keep the old amount.'),
        ],
        (context) async {
          final value = cents(amount.text);
          if (value == null) return false;
          return guard(
            context,
            () => app.changeLine(
              budget: budget,
              line: line.id,
              amount: value,
              repeat: repeat,
              from: day(from),
            ),
          );
        },
      );
    case 'end':
      var last = DateTime.now();
      var goesOn = false;
      return form(
        context,
        'End ${line.name}',
        (setState) => [
          SwitchListTile(
            title: const Text('Goes on'),
            value: goesOn,
            onChanged: (value) => setState(() => goesOn = value),
          ),
          if (!goesOn)
            dateButton(
              context,
              'Last day',
              last,
              (date) => setState(() => last = date),
            ),
        ],
        (context) => guard(
          context,
          () => app.endLine(
            budget: budget,
            line: line.id,
            lastDay: goesOn ? null : day(last),
          ),
        ),
      );
    default:
      if (!await confirm(context, 'Remove ${line.name}?') || !context.mounted) {
        return false;
      }
      return guard(
        context,
        () => app.removeLine(budget: budget, line: line.id),
      );
  }
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
    'New transaction',
    (setState) => [
      DropdownButton<LineRow?>(
        value: line,
        isExpanded: true,
        items: [
          const DropdownMenuItem(value: null, child: Text('Unplanned')),
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
      field(amount, 'Amount'),
      field(category, 'Category'),
      field(note, 'Note'),
      SwitchListTile(
        title: const Text('Income'),
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
            const DropdownMenuItem(value: null, child: Text('No pot')),
            for (final option in view.pots) ...[
              DropdownMenuItem(
                value: (option, false),
                child: Text('Into ${option.name}'),
              ),
              DropdownMenuItem(
                value: (option, true),
                child: Text('Paid from ${option.name}'),
              ),
            ],
          ],
          onChanged: (value) => setState(() {
            pot = value?.$1;
            fromPot = value?.$2 ?? false;
            if (value != null) income = false;
          }),
        ),
      dateButton(context, '', date, (picked) => setState(() => date = picked)),
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
    'New pot',
    (setState) => [
      field(name, 'Name'),
      field(goal, 'Goal (optional)'),
      SwitchListTile(
        title: const Text('By a date'),
        value: hasDate,
        onChanged: (value) => setState(() => hasDate = value),
      ),
      if (hasDate)
        dateButton(context, 'By', by, (date) => setState(() => by = date)),
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
