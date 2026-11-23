// A line, in full: its name and category, where a cost's money goes, its
// amounts over time as a table (the start, then each change), its end, and
// its removal. Every change is made by the Rust core, which keeps the
// line's due days.

import 'package:flutter/material.dart';

import 'forms.dart';
import 'look.dart';
import 'src/rust/api.dart';

class LineScreen extends StatefulWidget {
  const LineScreen({
    super.key,
    required this.app,
    required this.budget,
    required this.line,
    required this.currency,
    required this.pots,
    required this.destinations,
    this.onRestored,
  });

  final App app;
  final String budget;
  final String line;
  final String currency;
  // Where a cost's money can go: the budget's pots and the other budgets
  // (see `MonthView`).
  final List<PotRow> pots;
  final List<BudgetChoice> destinations;
  // Called if the line, once removed, is put back (Undo).
  final VoidCallback? onRestored;

  @override
  State<LineScreen> createState() => _LineScreenState();
}

class _LineScreenState extends State<LineScreen> {
  late Future<LineDetail> _detail;

  App get _app => widget.app;

  @override
  void initState() {
    super.initState();
    _load();
  }

  void _load() {
    setState(() {
      _detail = _app.lineDetail(budget: widget.budget, line: widget.line);
    });
  }

  /// Runs a form, and reloads if it changed something.
  Future<void> _then(Future<bool> changed) async {
    if (await changed) _load();
  }

  Future<bool> _rename(LineDetail line) {
    final name = TextEditingController(text: line.name);
    final category = TextEditingController(text: line.category);
    return form(
      context,
      context.words.rename,
      (setState) => [
        field(name, context.words.name),
        field(category, context.words.category),
      ],
      (context) => guard(
        context,
        () => _app.renameLine(
          budget: widget.budget,
          line: widget.line,
          name: name.text,
          category: category.text,
        ),
      ),
    );
  }

  /// The form for one amount: `row` to change it, or null for a new one.
  Future<bool> _amount(RevisionRow? row) {
    final amount = TextEditingController(
      text: row == null ? '' : centsText(row.amount),
    );
    // `null`: repeats as it did (as the amount before, for a new one).
    Repeat? repeat = row?.repeat;
    var from = row == null ? DateTime.now() : DateTime.parse(row.from);
    return form(
      context,
      row == null ? context.words.addChange : context.words.changeAmount,
      (setState) => [
        dateButton(
          context,
          context.words.from,
          from,
          (date) => setState(() => from = date),
        ),
        field(amount, context.words.amount),
        DropdownButton<Repeat?>(
          value: repeat,
          isExpanded: true,
          items: [
            DropdownMenuItem(value: null, child: Text(context.words.asBefore)),
            for (final option in Repeat.values)
              DropdownMenuItem(
                value: option,
                child: Text(repeatLabel(context, option)),
              ),
          ],
          onChanged: (value) => setState(() => repeat = value),
        ),
      ],
      (context) async {
        final value = cents(amount.text);
        if (value == null) return false;
        return guard(
          context,
          () => _app.setRevision(
            budget: widget.budget,
            line: widget.line,
            replacing: row?.from,
            from: day(from),
            amount: value,
            repeat: repeat,
          ),
        );
      },
    );
  }

  /// Where the money of `line`, a cost, goes now.
  GoesTo _goesTo(LineDetail line) => (
    widget.pots.where((pot) => pot.id == line.pot).firstOrNull,
    widget.destinations
        .where((other) => other.id == line.heldIn?.id)
        .firstOrNull,
  );

  String _goesToText(GoesTo goesTo) => switch (goesTo) {
    // Patterns on the record: `(final pot?, _)` matches a non-null pot.
    (final pot?, _) => context.words.feedsPot(pot.name),
    (_, final other?) => context.words.toBudget(other.name),
    _ => context.words.feedsNoPot,
  };

  /// The form to send `line`'s money elsewhere from a date. The line is
  /// cut in two there (by the core), so this screen closes after it.
  Future<void> _redirect(LineDetail line) async {
    var from = DateTime.now();
    var goesTo = _goesTo(line);
    final done = await form(
      context,
      context.words.whereItGoes,
      (setState) => [
        dateButton(
          context,
          context.words.from,
          from,
          (date) => setState(() => from = date),
        ),
        goesToChoice(
          context,
          widget.pots,
          widget.destinations,
          goesTo,
          (value) => setState(() => goesTo = value),
        ),
      ],
      (context) => guard(
        context,
        () => _app.redirectLine(
          budget: widget.budget,
          line: widget.line,
          from: day(from),
          pot: goesTo.$1?.id,
          toBudget: goesTo.$2?.id,
        ),
      ),
    );
    if (done && mounted) Navigator.of(context).pop();
  }

  Future<bool> _removeAmount(RevisionRow row) async {
    if (!await confirm(context, context.words.removeAmountQuestion) ||
        !mounted) {
      return false;
    }
    return guard(
      context,
      () => _app.removeRevision(
        budget: widget.budget,
        line: widget.line,
        from: row.from,
      ),
    );
  }

  Future<bool> _end(LineDetail line) {
    var last = line.endsOn == null
        ? DateTime.now()
        : DateTime.parse(line.endsOn!);
    var goesOn = line.endsOn == null;
    return form(
      context,
      context.words.endLine(line.name),
      (setState) => [
        SwitchListTile(
          title: Text(context.words.goesOn),
          value: goesOn,
          onChanged: (value) => setState(() => goesOn = value),
        ),
        if (!goesOn)
          dateButton(
            context,
            context.words.lastDay,
            last,
            (date) => setState(() => last = date),
          ),
      ],
      (context) => guard(
        context,
        () => _app.endLine(
          budget: widget.budget,
          line: widget.line,
          lastDay: goesOn ? null : day(last),
        ),
      ),
    );
  }

  Future<void> _removeLine(LineDetail line) async {
    if (!await confirm(context, context.words.removeQuestion(line.name)) ||
        !mounted) {
      return;
    }
    final removed = await guard(
      context,
      () => _app.removeLine(budget: widget.budget, line: widget.line),
    );
    if (removed && mounted) {
      // Offered from here, shown on the screen below once this one closes.
      offerUndo(context, _app, widget.budget, widget.line, widget.onRestored);
      Navigator.of(context).pop();
    }
  }

  @override
  Widget build(BuildContext context) {
    final words = context.words;
    return FutureBuilder(
      future: _detail,
      builder: (context, snapshot) {
        final line = snapshot.data;
        return Scaffold(
          appBar: AppBar(
            title: Text(line?.name ?? ''),
            actions: [
              if (line != null)
                IconButton(
                  onPressed: () => _removeLine(line),
                  icon: const Icon(Icons.delete_outline),
                  tooltip: words.removeIt,
                ),
            ],
          ),
          body: line == null
              ? Center(
                  child: snapshot.hasError
                      ? Text('${snapshot.error}')
                      : const CircularProgressIndicator(),
                )
              : _body(line),
        );
      },
    );
  }

  Widget _body(LineDetail line) {
    final words = context.words;
    final heading = Theme.of(context).textTheme.titleMedium;
    final rows = line.revisions;
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        if (line.heldIn != null)
          ListTile(
            leading: const Icon(Icons.link),
            title: Text(words.heldIn(line.heldIn!.name)),
          ),
        ListTile(
          leading: const Icon(Icons.edit),
          title: Text('${line.name} (${line.category})'),
          subtitle: Text('${words.name}, ${words.category}'),
          onTap: () => _then(_rename(line)),
        ),
        // An income's money goes nowhere else.
        if (!line.income &&
            (widget.pots.isNotEmpty || widget.destinations.isNotEmpty))
          ListTile(
            leading: const Icon(Icons.alt_route),
            title: Text(_goesToText(_goesTo(line))),
            subtitle: Text(words.redirect),
            onTap: () => _redirect(line),
          ),
        const SizedBox(height: 16),
        Text(words.amounts, style: heading),
        DataTable(
          showCheckboxColumn: false,
          columns: [
            const DataColumn(label: Text('')),
            DataColumn(label: Text(words.from)),
            DataColumn(label: Text(words.amount), numeric: true),
            DataColumn(label: Text(words.repeats)),
            const DataColumn(label: Text('')),
          ],
          rows: [
            // `indexed`: (position, row), the first being the start.
            for (final (index, row) in rows.indexed)
              DataRow(
                onSelectChanged: (_) => _then(_amount(row)),
                cells: [
                  DataCell(
                    Text(index == 0 ? words.start : words.changeNumber(index)),
                  ),
                  DataCell(Text(dateText(row.from))),
                  DataCell(Amount(row.amount, widget.currency)),
                  DataCell(
                    Text(
                      row.repeat == null
                          ? '—'
                          : repeatLabel(context, row.repeat!),
                    ),
                  ),
                  DataCell(
                    // The only amount can't go: remove the line instead.
                    rows.length == 1
                        ? const SizedBox.shrink()
                        : IconButton(
                            onPressed: () => _then(_removeAmount(row)),
                            icon: const Icon(Icons.close),
                            tooltip: words.removeAmount,
                          ),
                  ),
                ],
              ),
          ],
        ),
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton.icon(
            onPressed: () => _then(_amount(null)),
            icon: const Icon(Icons.add),
            label: Text(words.addChange),
          ),
        ),
        const SizedBox(height: 16),
        ListTile(
          leading: const Icon(Icons.event_busy),
          title: Text(
            line.endsOn == null
                ? words.goesOn
                : words.until(dateText(line.endsOn!)),
          ),
          subtitle: Text(words.endIt),
          onTap: () => _then(_end(line)),
        ),
      ],
    );
  }
}
