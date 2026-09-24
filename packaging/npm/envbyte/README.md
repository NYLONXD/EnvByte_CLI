# envbyte

End-to-end encrypted `.env` sharing for teams, with rotatable keys.

```bash
npm install -g envbyte
envbyte register
```

This package installs the prebuilt `envbyte` binary for your platform (macOS,
Linux and Windows on x64 or ARM64). Every `.env` file is encrypted on your
machine before it is uploaded, and each teammate gets their own sealed copy of
the project key, so nobody ever pastes a secret into chat.

- Website and other install methods: https://envbyte.trackedge.in
- Source, docs and issues: https://github.com/NYLONXD/EnvByte_CLI
