// A budget's trash (doc/scope.md: Corbeille): the lines and transactions
// deleted in the last 30 days, the latest first, each ready to be put
// back. The 30 days and the checks on restoring are the Rust core's.

import 'package:flutter/material.dart';

import 'forms.dart';
import 'look.dart';
import 'src/rust/api.dart';

class TrashScreen extends StatefulWidget {
  const TrashScreen({
    super.key,
    required this.app,
    required this.budget,
    required this.currency,
  });

  final App app;
  final String budget;
  final String currency;

  @override
  State<TrashScreen> createState() => _TrashScreenState();
}

class _TrashScreenState extends State<TrashScreen> {
  late Future<List<TrashRow>> _rows;

  @override
  void initState() {
    super.initState();
    _load();
  }

  void _load() {
    // A block, not `=> ...`: an arrow would return the Future assigned,
    // which `setState` refuses (see home.dart).
    setState(() {
      _rows = widget.app.trash(budget: widget.budget);
    });
  }

  Future<void> _restore(TrashRow row) async {
    final restored = await guard(
      context,
      () => widget.app.restore(budget: widget.budget, item: row.id),
    );
    if (restored && mounted) {
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(context.words.restored)));
      _load();
    }
  }

  /// Asks, then empties the trash: what is in it can't come back.
  Future<void> _empty() async {
    if (!await confirm(context, context.words.emptyTrashQuestion) || !mounted) {
      return;
    }
    final emptied = await guard(
      context,
      () => widget.app.emptyTrash(budget: widget.budget),
    );
    if (emptied) _load();
  }

  @override
  Widget build(BuildContext context) {
    final words = context.words;
    return Scaffold(
      appBar: AppBar(
        title: Text(words.trash),
        actions: [
          IconButton(
            onPressed: _empty,
            icon: const Icon(Icons.delete_forever),
            tooltip: words.emptyTrash,
          ),
        ],
      ),
      body: FutureBuilder(
        future: _rows,
        builder: (context, snapshot) {
          final rows = snapshot.data;
          if (rows == null) {
            return Center(
              child: snapshot.hasError
                  ? Text('${snapshot.error}')
                  : const CircularProgressIndicator(),
            );
          }
          if (rows.isEmpty) return Center(child: Text(words.trashEmpty));
          return ListView(
            children: [
              for (final row in rows)
                ListTile(
                  // A line, or a transaction with its date.
                  leading: row.line
                      ? const Icon(Icons.view_list)
                      : Text(dateText(row.date!)),
                  title: Text(
                    [
                      row.name,
                      if (row.detail.isNotEmpty) row.detail,
                    ].join(' · '),
                  ),
                  subtitle: Text(words.deletedOn(dateText(row.deletedOn))),
                  trailing: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Amount(
                        row.income ? row.amount : -row.amount,
                        widget.currency,
                      ),
                      IconButton(
                        onPressed: () => _restore(row),
                        icon: const Icon(Icons.restore_from_trash),
                        tooltip: words.restore,
                      ),
                    ],
                  ),
                ),
            ],
          );
        },
      ),
    );
  }
}
