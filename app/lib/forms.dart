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

/// Who added `item` (a line or a transaction shown in `budget`) and who
/// changed it last, in small print (doc/scope.md: Traçabilité). Nothing
/// while it loads, or if it can't be found.
class TraceText extends StatefulWidget {
  const TraceText({
    super.key,
    required this.app,
    required this.budget,
    required this.item,
  });

  final App app;
  final String budget;
  final String item;

  @override
  State<TraceText> createState() => _TraceTextState();
}

class _TraceTextState extends State<TraceText> {
  late final Future<Trace> _trace = widget.app.trace(
    budget: widget.budget,
    item: widget.item,
  );

  @override
  Widget build(BuildContext context) {
    final words = context.words;
    // "you", or the member's name.
    String who(Stamp stamp) => stamp.me ? words.you : stamp.by;
    return FutureBuilder(
      future: _trace,
      builder: (context, snapshot) {
        final trace = snapshot.data;
        if (trace == null) return const SizedBox.shrink();
        final changed = trace.changed;
        return Text(
          [
            words.addedBy(who(trace.added), timeText(trace.added.time)),
            if (changed != null)
              words.changedBy(who(changed), timeText(changed.time)),
          ].join('\n'),
          style: Theme.of(context).textTheme.bodySmall,
        );
      },
    );
  }
}

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

/// Where a cost's money goes: a pot, another budget, or neither. A record
/// `(pot, budget)` with at most one set; records compare by value, as a
/// dropdown needs. `typedef` names a type, like C++'s `using`.
typedef GoesTo = (PotRow?, BudgetChoice?);

/// A choice of where a cost's money goes: nowhere, one of `pots`, or one
/// of `destinations` (other budgets).
Widget goesToChoice(
  BuildContext context,
  List<PotRow> pots,
  List<BudgetChoice> destinations,
  GoesTo value,
  void Function(GoesTo) changed,
) {
  return DropdownButton<GoesTo>(
    value: value,
    isExpanded: true,
    items: [
      DropdownMenuItem(
        value: (null, null),
        child: Text(context.words.feedsNoPot),
      ),
      for (final pot in pots)
        DropdownMenuItem(
          value: (pot, null),
          child: Text(context.words.feedsPot(pot.name)),
        ),
      for (final other in destinations)
        DropdownMenuItem(
          value: (null, other),
          child: Text(context.words.toBudget(other.name)),
        ),
    ],
    onChanged: (choice) => changed(choice ?? (null, null)),
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
  GoesTo goesTo = (null, null);
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
        goesToChoice(
          context,
          view.pots,
          view.destinations,
          goesTo,
          // Money going somewhere is a cost.
          (value) => setState(() {
            goesTo = value;
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
/// lines, or moving money into or out of a pot; or, given `existing`, to
/// change that one, with a button to remove it instead. One held by
/// another budget (`reflected`) only changes in date, amount and note.
/// Returns whether something changed.
Future<bool> addTransaction(
  BuildContext context,
  App app,
  String budget,
  MonthView view, {
  TransactionRow? existing,
  VoidCallback? onRestored,
}) async {
  final amount = TextEditingController(
    text: existing == null ? '' : centsText(existing.amount),
  );
  final category = TextEditingController(text: existing?.category ?? '');
  final note = TextEditingController(text: existing?.note ?? '');
  var income = existing?.income ?? false;
  // The line, by identifier: a transaction's line may not be one of this
  // month's (paid early, say), and must not be lost by editing it.
  String? lineId = existing?.lineId;
  // The pot, and whether the money comes from it (else goes into it).
  PotRow? pot = view.pots.where((p) => p.id == existing?.pot).firstOrNull;
  var fromPot = existing?.fromPot ?? false;
  var date = existing == null ? DateTime.now() : DateTime.parse(existing.date);
  final all = existing == null || !existing.reflected;
  var remove = false;
  final changed = await form(
    context,
    existing == null
        ? context.words.newTransaction
        : context.words.changeTransaction,
    (setState) => [
      if (all)
        DropdownButton<String?>(
          value: lineId,
          isExpanded: true,
          items: [
            DropdownMenuItem(value: null, child: Text(context.words.unplanned)),
            for (final option in view.lines)
              DropdownMenuItem(value: option.id, child: Text(option.name)),
            if (lineId != null && !view.lines.any((l) => l.id == lineId))
              DropdownMenuItem(
                value: lineId,
                child: Text(existing?.line ?? '?'),
              ),
          ],
          // Choosing a line fills in its kind, category and pot.
          onChanged: (value) => setState(() {
            lineId = value;
            final chosen = view.lines.where((l) => l.id == value).firstOrNull;
            if (chosen != null) {
              income = chosen.income;
              category.text = chosen.category;
              pot = view.pots.where((p) => p.id == chosen.pot).firstOrNull;
              fromPot = false;
            }
          }),
        ),
      field(amount, context.words.amount),
      if (all) field(category, context.words.category),
      field(note, context.words.note),
      if (all)
        SwitchListTile(
          title: Text(context.words.income),
          value: income,
          onChanged: lineId == null && pot == null
              ? (value) => setState(() => income = value)
              : null,
        ),
      if (all && view.pots.isNotEmpty)
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
      if (existing != null) ...[
        Align(
          alignment: Alignment.centerLeft,
          child: TraceText(app: app, budget: budget, item: existing.id),
        ),
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton.icon(
            icon: const Icon(Icons.delete_outline),
            label: Text(context.words.removeIt),
            // Closes the form; the removal is asked for below.
            onPressed: () {
              remove = true;
              Navigator.of(context).pop(false);
            },
          ),
        ),
      ],
    ],
    (context) async {
      final value = cents(amount.text);
      if (value == null) return false;
      return guard(
        context,
        () => existing == null
            ? app.addTransaction(
                budget: budget,
                dateText: day(date),
                income: income,
                amount: value,
                category: category.text,
                line: lineId,
                note: note.text,
                pot: pot?.id,
                fromPot: fromPot,
              )
            : app.changeTransaction(
                budget: budget,
                transaction: existing.id,
                dateText: day(date),
                income: income,
                amount: value,
                category: category.text,
                line: lineId,
                note: note.text,
                pot: pot?.id,
                fromPot: fromPot,
              ),
      );
    },
  );
  if (remove && context.mounted) {
    return removeTransaction(
      context,
      app,
      budget,
      existing!,
      onRestored: onRestored,
    );
  }
  return changed;
}

/// Asks, then removes a transaction. A reflection is removed from the
/// budget it comes from (its category is that budget's name).
Future<bool> removeTransaction(
  BuildContext context,
  App app,
  String budget,
  TransactionRow transaction, {
  VoidCallback? onRestored,
}) async {
  final question = transaction.reflected
      ? context.words.removeThereToo(transaction.category)
      : context.words.removeTransactionQuestion;
  if (!await confirm(context, question) || !context.mounted) {
    return false;
  }
  final removed = await guard(
    context,
    () => app.removeTransaction(budget: budget, transaction: transaction.id),
  );
  if (removed && context.mounted) {
    offerUndo(context, app, budget, transaction.id, onRestored);
  }
  return removed;
}

/// Says that something was removed, with a button to put item `item` back
/// (from the trash), then calls `onRestored`.
void offerUndo(
  BuildContext context,
  App app,
  String budget,
  String item,
  VoidCallback? onRestored,
) {
  // The messenger outlives the screen (it belongs to the app), so the
  // message can stay after a screen closes.
  final messenger = ScaffoldMessenger.of(context);
  final words = context.words;
  messenger.showSnackBar(
    SnackBar(
      content: Text(words.removed),
      action: SnackBarAction(
        label: words.undo,
        onPressed: () async {
          try {
            await app.restore(budget: budget, item: item);
            onRestored?.call();
          } catch (error) {
            messenger.showSnackBar(SnackBar(content: Text('$error')));
          }
        },
      ),
    ),
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
