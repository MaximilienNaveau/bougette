// The settings: this member's name, the server, recovery, the member's
// other devices, and joining a budget with an invitation. Every action is
// the same as the command-line client's, done by the Rust core.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'forms.dart';
import 'look.dart';
import 'src/rust/api.dart';
import 'src/rust/api/members.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key, required this.app});

  final App app;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

/// What the screen shows, read together.
class _State {
  const _State(this.name, this.server, this.recovery);

  final String? name;
  final ServerInfo? server;
  final bool recovery;
}

class _SettingsScreenState extends State<SettingsScreen> {
  late Future<_State> _state;

  @override
  void initState() {
    super.initState();
    _load();
  }

  void _load() {
    setState(() {
      _state = () async {
        final app = widget.app;
        return _State(
          await app.name(),
          await app.server(),
          await app.hasRecovery(),
        );
      }();
    });
  }

  /// Runs `action` and reloads; shows `done` if it succeeded.
  Future<void> _run(Future<Object?> Function() action, [String? done]) async {
    final ok = await guard(context, action);
    if (ok && done != null && mounted) {
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(done)));
    }
    _load();
  }

  String _synced(SyncSummary summary) =>
      context.words.synced('${summary.sent}', '${summary.received}');

  Future<void> _rename(String? current) async {
    final name = await askText(
      context,
      context.words.yourName,
      context.words.name,
      initial: current ?? '',
    );
    if (name == null || name.isEmpty || !mounted) return;
    await _run(() => widget.app.setName(name: name));
  }

  Future<void> _join() async {
    final address = TextEditingController();
    final token = TextEditingController();
    final username = TextEditingController();
    SyncSummary? summary;
    final done = await form(
      context,
      context.words.joinServer,
      (setState) => [
        field(address, context.words.address),
        field(token, context.words.invitationToken),
        field(username, context.words.username),
      ],
      (context) => guard(context, () async {
        summary = await widget.app.joinServer(
          address: address.text.trim(),
          token: token.text.trim(),
          username: username.text.trim(),
        );
      }),
    );
    if (done && mounted && summary != null) {
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(_synced(summary!))));
    }
    _load();
  }

  Future<void> _sync() async {
    SyncSummary? summary;
    final ok = await guard(
      context,
      () async => summary = await widget.app.syncNow(),
    );
    if (ok && mounted) {
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(_synced(summary!))));
    }
  }

  Future<void> _setUpRecovery() async {
    final own = TextEditingController();
    String? generated;
    final done = await form(
      context,
      context.words.setUpRecovery,
      (setState) => [field(own, context.words.ownPhrase)],
      (context) => guard(context, () async {
        final phrase = own.text.trim();
        generated = await widget.app.setUpRecovery(
          phrase: phrase.isEmpty ? null : phrase,
        );
      }),
    );
    if (done && generated != null && mounted) {
      await _show(context.words.phrase, generated!, context.words.writePhrase);
    }
    _load();
  }

  Future<void> _recover() async {
    final address = TextEditingController();
    final token = TextEditingController();
    final username = TextEditingController();
    final phrase = TextEditingController();
    await form(
      context,
      context.words.recover,
      (setState) => [
        field(address, context.words.address),
        field(token, context.words.invitationToken),
        field(username, context.words.username),
        field(phrase, context.words.phrase),
      ],
      (context) => guard(
        context,
        () => widget.app.recover(
          address: address.text.trim(),
          token: token.text.trim(),
          username: username.text.trim(),
          phrase: phrase.text,
        ),
      ),
    );
    _load();
  }

  Future<void> _showCode() async {
    String? code;
    final ok = await guard(
      context,
      () async => code = await widget.app.deviceCode(),
    );
    if (ok && mounted) {
      await _show(context.words.code, code!, context.words.codeWarning);
    }
  }

  Future<void> _link() async {
    final code = await askText(
      context,
      context.words.linkThis,
      context.words.code,
    );
    if (code == null || code.isEmpty || !mounted) return;
    await _run(() => widget.app.linkDevice(code: code.trim()));
  }

  Future<void> _accept() async {
    final invitation = TextEditingController();
    final username = TextEditingController();
    final done = await form(
      context,
      context.words.acceptInvitation,
      (setState) => [
        field(invitation, context.words.invitation),
        field(username, context.words.usernameIfNew),
      ],
      (context) => guard(
        context,
        () => widget.app.accept(
          invitation: invitation.text.trim(),
          username: username.text.trim().isEmpty ? null : username.text.trim(),
        ),
      ),
    );
    if (done && mounted) {
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(context.words.askedToJoin)));
    }
    _load();
  }

  /// Shows a secret to copy (a phrase, a code), with a warning.
  Future<void> _show(String title, String secret, String warning) {
    return showDialog<void>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(title),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Selectable, and in a fixed-width font: easy to copy out.
            SelectableText(
              secret,
              style: const TextStyle(fontFamily: 'monospace'),
            ),
            const SizedBox(height: 12),
            Text(warning),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () async {
              await Clipboard.setData(ClipboardData(text: secret));
              if (context.mounted) {
                ScaffoldMessenger.of(
                  context,
                ).showSnackBar(SnackBar(content: Text(context.words.copied)));
              }
            },
            child: Text(context.words.copy),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(),
            child: Text(context.words.close),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final words = context.words;
    return Scaffold(
      appBar: AppBar(title: Text(words.settings)),
      body: FutureBuilder(
        future: _state,
        builder: (context, snapshot) {
          if (snapshot.hasError) {
            return Center(child: Text('${snapshot.error}'));
          }
          final state = snapshot.data;
          if (state == null) {
            return const Center(child: CircularProgressIndicator());
          }
          final server = state.server;
          final heading = Theme.of(context).textTheme.titleMedium;
          return ListView(
            padding: const EdgeInsets.all(16),
            children: [
              ListTile(
                leading: const Icon(Icons.badge),
                title: Text(state.name ?? words.noNameSet),
                subtitle: Text(words.yourName),
                onTap: () => _rename(state.name),
              ),
              const Divider(),
              Text(words.server, style: heading),
              ListTile(
                leading: const Icon(Icons.dns),
                title: Text(
                  server == null
                      ? words.noServer
                      : words.onServer(
                          server.username,
                          server.address.split('@').first,
                        ),
                ),
              ),
              if (server == null)
                ListTile(
                  leading: const Icon(Icons.login),
                  title: Text(words.joinServer),
                  onTap: _join,
                )
              else
                ListTile(
                  leading: const Icon(Icons.sync),
                  title: Text(words.syncNow),
                  onTap: _sync,
                ),
              ListTile(
                leading: const Icon(Icons.group_add),
                title: Text(words.acceptInvitation),
                onTap: _accept,
              ),
              const Divider(),
              Text(words.recovery, style: heading),
              ListTile(
                leading: Icon(
                  state.recovery ? Icons.lock : Icons.warning_amber,
                ),
                title: Text(
                  state.recovery ? words.recoveryOn : words.recoveryOff,
                ),
              ),
              if (server != null)
                ListTile(
                  leading: const Icon(Icons.key),
                  title: Text(words.setUpRecovery),
                  onTap: _setUpRecovery,
                ),
              ListTile(
                leading: const Icon(Icons.restore),
                title: Text(words.recover),
                onTap: _recover,
              ),
              const Divider(),
              Text(words.devices, style: heading),
              ListTile(
                leading: const Icon(Icons.qr_code),
                title: Text(words.showCode),
                onTap: _showCode,
              ),
              ListTile(
                leading: const Icon(Icons.link),
                title: Text(words.linkThis),
                onTap: _link,
              ),
            ],
          );
        },
      ),
    );
  }
}
