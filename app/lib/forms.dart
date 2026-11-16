// The small forms: a budget's name, a new line, a new transaction. They
// only collect what the user types and forward it; Rust checks it.

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

/// Asks for one text; null if cancelled.
Future<String?> askText(BuildContext context, String title, String label) {
  final controller = TextEditingController();
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

/// Reads `12`, `12.5` or `12,34` as cents; null if it isn't an amount.
/// (Parsing what a person typed, not a budget rule.)
int? cents(String text) {
  final match = RegExp(r'^(\d+)(?:[.,](\d{1,2}))?$').firstMatch(text.trim());
  if (match == null) return null;
  final units = int.parse(match.group(1)!);
  final rest = (match.group(2) ?? '').padRight(2, '0');
  return units * 100 + int.parse(rest);
}

/// `2026-11-05` for a date.
String day(DateTime date) =>
    '${date.year}-${'${date.month}'.padLeft(2, '0')}-${'${date.day}'.padLeft(2, '0')}';

/// The form to add a planned line. Returns whether one was added.
Future<bool> addLine(BuildContext context, App app, String budget) async {
  final name = TextEditingController();
  final category = TextEditingController();
  final amount = TextEditingController();
  var income = false;
  var repeat = Repeat.monthly;
  var first = DateTime.now();
  final added = await showDialog<bool>(
    context: context,
    // `StatefulBuilder`: a dialog with its own state (the choices made).
    builder: (context) => StatefulBuilder(
      builder: (context, setState) => AlertDialog(
        title: const Text('New line'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              controller: name,
              decoration: const InputDecoration(labelText: 'Name'),
            ),
            TextField(
              controller: category,
              decoration: const InputDecoration(labelText: 'Category'),
            ),
            TextField(
              controller: amount,
              decoration: const InputDecoration(labelText: 'Amount'),
            ),
            SwitchListTile(
              title: const Text('Income'),
              value: income,
              onChanged: (value) => setState(() => income = value),
            ),
            DropdownButton<Repeat>(
              value: repeat,
              isExpanded: true,
              items: [
                for (final option in Repeat.values)
                  DropdownMenuItem(value: option, child: Text(option.name)),
              ],
              onChanged: (value) => setState(() => repeat = value ?? repeat),
            ),
            TextButton.icon(
              icon: const Icon(Icons.event),
              label: Text('First due ${day(first)}'),
              onPressed: () async {
                final picked = await showDatePicker(
                  context: context,
                  initialDate: first,
                  firstDate: DateTime(2000),
                  lastDate: DateTime(2100),
                );
                if (picked != null) setState(() => first = picked);
              },
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () async {
              final value = cents(amount.text);
              if (value == null) return;
              final ok = await guard(
                context,
                () => app.addLine(
                  budget: budget,
                  name: name.text,
                  category: category.text,
                  income: income,
                  amount: value,
                  repeat: repeat,
                  first: day(first),
                ),
              );
              if (ok && context.mounted) Navigator.of(context).pop(true);
            },
            child: const Text('Add'),
          ),
        ],
      ),
    ),
  );
  return added ?? false;
}

/// The form to add a transaction, possibly realising one of the month's
/// lines. Returns whether one was added.
Future<bool> addTransaction(
  BuildContext context,
  App app,
  String budget,
  MonthView view,
) async {
  final amount = TextEditingController();
  final category = TextEditingController();
  final note = TextEditingController();
  var income = false;
  LineRow? line;
  var date = DateTime.now();
  final added = await showDialog<bool>(
    context: context,
    builder: (context) => StatefulBuilder(
      builder: (context, setState) => AlertDialog(
        title: const Text('New transaction'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            DropdownButton<LineRow?>(
              value: line,
              isExpanded: true,
              hint: const Text('Unplanned'),
              items: [
                const DropdownMenuItem(value: null, child: Text('Unplanned')),
                for (final option in view.lines)
                  DropdownMenuItem(value: option, child: Text(option.name)),
              ],
              // Choosing a line fills in its kind and category.
              onChanged: (value) => setState(() {
                line = value;
                if (value != null) {
                  income = value.income;
                  category.text = value.category;
                }
              }),
            ),
            TextField(
              controller: amount,
              decoration: const InputDecoration(labelText: 'Amount'),
            ),
            TextField(
              controller: category,
              decoration: const InputDecoration(labelText: 'Category'),
            ),
            TextField(
              controller: note,
              decoration: const InputDecoration(labelText: 'Note'),
            ),
            SwitchListTile(
              title: const Text('Income'),
              value: income,
              onChanged: line == null
                  ? (value) => setState(() => income = value)
                  : null,
            ),
            TextButton.icon(
              icon: const Icon(Icons.event),
              label: Text(day(date)),
              onPressed: () async {
                final picked = await showDatePicker(
                  context: context,
                  initialDate: date,
                  firstDate: DateTime(2000),
                  lastDate: DateTime(2100),
                );
                if (picked != null) setState(() => date = picked);
              },
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () async {
              final value = cents(amount.text);
              if (value == null) return;
              final ok = await guard(
                context,
                () => app.addTransaction(
                  budget: budget,
                  dateText: day(date),
                  income: income,
                  amount: value,
                  category: category.text,
                  line: line?.id,
                  note: note.text,
                ),
              );
              if (ok && context.mounted) Navigator.of(context).pop(true);
            },
            child: const Text('Add'),
          ),
        ],
      ),
    ),
  );
  return added ?? false;
}
