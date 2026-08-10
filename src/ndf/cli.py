import argparse
import json
from collections.abc import Sequence

from .version import (
    CLI_STABILITY,
    FORMAT_VERSION,
    IR_VERSION,
    PLUGIN_API,
    TOOL_VERSION,
)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="ndf")
    parser.add_argument("--machine-version", action="store_true")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    if args.machine_version:
        print(
            json.dumps(
                {
                    "cli_stability": CLI_STABILITY,
                    "format_version": FORMAT_VERSION,
                    "ir_version": IR_VERSION,
                    "plugin_api": PLUGIN_API,
                    "tool_version": TOOL_VERSION,
                },
                sort_keys=True,
            )
        )
    return 0


def entrypoint() -> None:
    raise SystemExit(main())
