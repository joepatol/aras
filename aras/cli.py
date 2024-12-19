import importlib
import os
import sys

import click
import aras
from aras import LogLevel


@click.group()
def cli() -> None:
    pass


@cli.command()
@click.argument('application', type=click.STRING)
@click.option(
    "--host",
    type=str,
    default="127.0.0.1",
    help="Bind socket to this host.",
    show_default=True,
)
@click.option(
    "--port",
    type=int,
    default=8080,
    help="Bind socket to this port.",
    show_default=True,
)
@click.option(
    "--log-level",
    type=str,
    default="INFO",
    help="Set the server log level",
    show_default=True,
)
def serve(application: str, host: str, port: int, log_level: LogLevel) -> None:
    sys.path.insert(0, os.getcwd())
    module_str, application_str = application.split(":")
    module = importlib.import_module(module_str)
    loaded_app = getattr(module, application_str)
    aras.serve(
        loaded_app,
        addr=[int(i) for i in host.split(".")],
        port=port,
        log_level=log_level,
    )
