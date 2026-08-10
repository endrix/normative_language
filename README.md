# NDF

NDF is an experimental knowledge compiler for normative engineering documents.
Markdown, ASL, source, test, and evidence files remain Git-tracked sources of
truth; generated indexes are disposable query artifacts.

## Development

NDF requires Python 3.11 or newer.

```bash
python -m pip install -r requirements-lock.txt
python -m pip install -e . --no-deps
python -m unittest discover -s tests -p 'test_*.py'
```

Print the independently versioned public surfaces with:

```bash
ndf --machine-version
```

## Documentation

- [English specification](normative_language.md)
- [Chinese specification](normative_language_cn.md)
- [Format 0.1 to 0.2 migration](docs/format/migration-v0.1-to-v0.2.md)
- [PTO ASL adapter](docs/adapters/pto-asl.md)

Format 0.2, canonical IR 0.1, JSON output schema 0.1, CLI spelling, and the
plugin API are experimental. Downstream repositories consume NDF from an exact
Git commit recorded in `ndf.lock`; CI must not fetch an unpinned branch.
