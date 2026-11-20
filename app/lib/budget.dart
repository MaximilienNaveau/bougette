// A budget, in one screen: its year's twelve months at the top
// (doc/scope.md: the current year by default, moving year by year back to
// its first entry and without limit forward, each month coloured by where its figures come from and by any
// overspending), in rows that fit the window, and below it the selected
// month's lines, totals, transactions and pots, ready to edit. It opens on
// the current month.

import 'package:flutter/material.dart';

import 'forms.dart';
import 'line.dart';
import 'look.dart';
import 'members.dart';
import 'month.dart';
import 'src/rust/api.dart';

class BudgetScreen extends StatefulWidget {
  const BudgetScreen({super.key, required this.app, required this.budget});

  final App app;
  final BudgetSummary budget;

  @override
  State<BudgetScreen> createState() => _BudgetScreenState();
}

/// The width of a month in the strip, its gap included.
const double _tileWidth = 128;

/// The narrowest a month can be in a row of months.
const double _minTileWidth = 104;

/// The height of the strip of months, its padding included.
const double _tileHeight = 132;

class _BudgetScreenState extends State<BudgetScreen> {
  final DateTime _now = DateTime.now();
  late int _year = _now.year;
  // The month shown below the strip: 1 to 12.
  late int _month = _now.month;
  late Future<List<MonthCell>> _months;
  late Future<MonthView> _view;
  // The name and flag as changed here (the summary given is a snapshot).
  late String _name = widget.budget.name;
  late bool _personal = widget.budget.personal;
  // Starts with the month shown in view (two months before it at the
  // left edge); the list applies it, within its limits, when it appears.
  late final _scroll = ScrollController(
    initialScrollOffset: (_now.month - 3) * _tileWidth,
  );

  String get _budget => widget.budget.id;

  /// `2026-11` for the month shown.
  String get _selected => '$_year-${'$_month'.padLeft(2, '0')}';

  @override
  void initState() {
    super.initState();
    _reload();
  }

  @override
  void dispose() {
    _scroll.dispose();
    super.dispose();
  }

  /// Reads the calendar and the month shown again (after any change).
  void _reload() {
    setState(() {
      _months = widget.app.calendar(budget: _budget, year: _year);
      _view = widget.app.month(budget: _budget, month: _selected);
    });
  }

  void _select(int month) {
    _month = month;
    _reload();
  }

  void _moveYear(int by) {
    _year += by;
    _reload();
  }

  /// Runs a form, and reloads if it changed something.
  Future<void> _then(Future<bool> changed) async {
    if (await changed) _reload();
  }

  /// Opens a line of the month `view` shows in full, and reloads when
  /// back from it.
  Future<void> _openLine(LineRow line, MonthView view) async {
    await Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => LineScreen(
          app: widget.app,
          budget: _budget,
          line: line.id,
          currency: view.currency,
          pots: view.pots,
          destinations: view.destinations,
        ),
      ),
    );
    _reload();
  }

  Future<void> _rename() async {
    final name = await askText(
      context,
      context.words.rename,
      context.words.name,
      initial: _name,
    );
    if (name == null || name.isEmpty || !mounted) return;
    final ok = await guard(
      context,
      () => widget.app.renameBudget(budget: _budget, name: name),
    );
    if (ok) setState(() => _name = name);
  }

  Future<void> _members() async {
    await Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => MembersScreen(app: widget.app, budget: _budget),
      ),
    );
    _reload();
  }

  Future<void> _togglePersonal() async {
    final ok = await guard(
      context,
      () => widget.app.setPersonal(budget: _budget, personal: !_personal),
    );
    if (ok) {
      _personal = !_personal;
      _reload(); // a personal budget shows its reflections
    }
  }

  @override
  Widget build(BuildContext context) {
    final app = widget.app;
    final currency = widget.budget.currency;
    return FutureBuilder(
      future: _view,
      builder: (context, snapshot) {
        final view = snapshot.data;
        return Scaffold(
          appBar: AppBar(
            title: Text(_name),
            actions: [
              // A menu of the budget's settings.
              PopupMenuButton<VoidCallback>(
                onSelected: (action) => action(),
                itemBuilder: (context) => [
                  PopupMenuItem(
                    value: _members,
                    child: Text(context.words.members),
                  ),
                  PopupMenuItem(
                    value: _rename,
                    child: Text(context.words.rename),
                  ),
                  CheckedPopupMenuItem(
                    value: _togglePersonal,
                    checked: _personal,
                    child: Text(context.words.personalBudget),
                  ),
                ],
              ),
            ],
          ),
          floatingActionButton: view == null
              ? null
              : Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    FloatingActionButton.small(
                      heroTag: 'line',
                      tooltip: context.words.addLine,
                      onPressed: () =>
                          _then(addLine(context, app, _budget, view)),
                      child: const Icon(Icons.playlist_add),
                    ),
                    const SizedBox(height: 8),
                    FloatingActionButton.extended(
                      heroTag: 'transaction',
                      onPressed: () =>
                          _then(addTransaction(context, app, _budget, view)),
                      icon: const Icon(Icons.add),
                      label: Text(context.words.transactionButton),
                    ),
                  ],
                ),
          body: Column(
            children: [
              _years(view),
              _strip(currency),
              const Divider(height: 1),
              // `Expanded`: the month takes the rest of the height.
              Expanded(
                child: snapshot.hasError
                    ? Center(child: Text('${snapshot.error}'))
                    : view == null
                    ? const Center(child: CircularProgressIndicator())
                    : MonthBody(
                        view: view,
                        onLine: (line) => _openLine(line, view),
                        onTransaction: (transaction) => _then(
                          addTransaction(
                            context,
                            app,
                            _budget,
                            view,
                            existing: transaction,
                          ),
                        ),
                        onAddPot: () => _then(addPot(context, app, _budget)),
                      ),
              ),
            ],
          ),
        );
      },
    );
  }

  /// The year shown, between buttons to the years before and after, each
  /// naming the year it goes to: right above the months it changes, and
  /// apart from the screen's back arrow.
  Widget _years(MonthView? view) {
    final text = Theme.of(context).textTheme;
    return Padding(
      padding: const EdgeInsets.only(top: 8),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          TextButton.icon(
            // Not before the year of the budget's first entry (null
            // `onPressed` greys the button out).
            onPressed: view != null && _year <= view.firstYear
                ? null
                : () => _moveYear(-1),
            icon: const Icon(Icons.chevron_left),
            label: Text('${_year - 1}'),
          ),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Text('$_year', style: text.titleLarge),
          ),
          TextButton.icon(
            onPressed: () => _moveYear(1),
            // The arrow after the year, pointing where it goes.
            iconAlignment: IconAlignment.end,
            icon: const Icon(Icons.chevron_right),
            label: Text('${_year + 1}'),
          ),
        ],
      ),
    );
  }

  /// The twelve months of the year, all in view: in one row, or in rows
  /// of 6 or 4 if the window is narrower. On a phone, where even 4 don't
  /// fit, a strip to swipe.
  Widget _strip(String currency) {
    return FutureBuilder(
      future: _months,
      builder: (context, snapshot) {
        final months = snapshot.data;
        if (months == null) {
          return SizedBox(
            height: _tileHeight,
            child: Center(
              child: Text(snapshot.hasError ? '${snapshot.error}' : ''),
            ),
          );
        }
        // `indexed` gives (position, item) pairs, like Python's enumerate.
        final tiles = [
          for (final (index, cell) in months.indexed)
            _MonthTile(
              cell: cell,
              currency: currency,
              selected: index + 1 == _month,
              onTap: () => _select(index + 1),
            ),
        ];
        // `LayoutBuilder` gives the width available, like a resize event.
        return LayoutBuilder(
          builder: (context, constraints) {
            final width = constraints.maxWidth - 16; // the padding
            // The most months per row that divides the year evenly and
            // leaves each tile wide enough. `firstWhere` with `orElse`:
            // `std::find_if`, with a default when nothing matches.
            final perRow = const [
              12,
              6,
              4,
            ].firstWhere((n) => width / n >= _minTileWidth, orElse: () => 0);
            if (perRow == 0) {
              return SizedBox(
                height: _tileHeight,
                child: ListView(
                  controller: _scroll,
                  scrollDirection: Axis.horizontal,
                  padding: const EdgeInsets.all(8),
                  children: [
                    for (final tile in tiles)
                      SizedBox(width: _tileWidth, child: tile),
                  ],
                ),
              );
            }
            return Padding(
              padding: const EdgeInsets.all(8),
              child: Column(
                children: [
                  // Rows of `perRow` tiles, each tile an equal share of
                  // the width (`Expanded`).
                  for (var start = 0; start < 12; start += perRow)
                    SizedBox(
                      height: _tileHeight - 16,
                      child: Row(
                        children: [
                          for (final tile in tiles.sublist(
                            start,
                            start + perRow,
                          ))
                            Expanded(child: tile),
                        ],
                      ),
                    ),
                ],
              ),
            );
          },
        );
      },
    );
  }
}

/// One month of the strip: its name, where its figures come from, its
/// expected balance (coloured if overspent) and the running balance.
class _MonthTile extends StatelessWidget {
  const _MonthTile({
    required this.cell,
    required this.currency,
    required this.selected,
    required this.onTap,
  });

  final MonthCell cell;
  final String currency;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final overspent = statusColour(cell.status);
    final colours = Theme.of(context).colorScheme;
    final text = Theme.of(context).textTheme;
    return Card(
      color: sourceColour(context, cell.source),
      // The month shown gets a thick border; an overspent one a border in
      // its status colour.
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: selected
            ? BorderSide(color: overspent ?? colours.primary, width: 4)
            : overspent == null
            ? BorderSide.none
            : BorderSide(color: overspent, width: 2),
      ),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(12),
        child: Padding(
          padding: const EdgeInsets.all(8),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _fit(
                Text(
                  monthTitle(cell.month, withYear: false),
                  style: text.titleSmall?.copyWith(
                    fontWeight: selected ? FontWeight.bold : null,
                  ),
                ),
                Alignment.centerLeft,
              ),
              _fit(
                Text(sourceLabel(context, cell.source), style: text.labelSmall),
                Alignment.centerLeft,
              ),
              const Spacer(),
              _fit(
                Amount(cell.expected, currency, status: cell.status),
                Alignment.centerRight,
              ),
              _fit(
                Text(money(cell.running, currency), style: text.bodySmall),
                Alignment.centerRight,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// `child` on one line, shrunk if it's too wide (a large amount) rather
/// than wrapped onto the next line.
Widget _fit(Widget child, Alignment alignment) =>
    FittedBox(fit: BoxFit.scaleDown, alignment: alignment, child: child);
