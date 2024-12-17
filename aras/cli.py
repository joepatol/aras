import importlib

import click
import aras


@click.group()
def cli() -> None:
    pass


@cli.command()
@click.argument('application', type=click.STRING)
def serve(application: str) -> None:
    module_str, application_str = application.split(":")
    module = importlib.import_module(module_str)
    loaded_app = getattr(module, application_str)
    aras.serve(loaded_app)
