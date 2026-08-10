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
