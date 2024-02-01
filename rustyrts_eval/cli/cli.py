import logging

import click

from .analyze.commands import analyze
from .db.commands import db
from .evaluate.commands import evaluate
from .. import __version__
from ..util.logging.logger import configure_logging_verbosity

# debug
DEBUG_MODE_MSG = "Debug mode is on."

# status
STATUS_MSG = "Running version {}.".format(__version__)

# input validation
INVALID_PARAMETERS_MSG = "Invalid parameters provided."


@click.group(name="rts_eval")
@click.pass_context
@click.option("--debug", is_flag=True, default=False, help="Show debug information.")
def entry_point(ctx, debug):
    """
    rts_eval CLI
    """
    ctx.ensure_object(dict)
    ctx.obj["debug"] = debug

    # set logging level
    if debug:
        configure_logging_verbosity(verbosity=logging.DEBUG)
        click.echo(click.style(DEBUG_MODE_MSG, fg="red", bold=True))
    else:
        configure_logging_verbosity(verbosity=logging.INFO)


@entry_point.command()
@click.pass_obj
def version(ctx):
    """
    Get version.
    """
    click.echo(click.style(STATUS_MSG, fg="green", bold=True))


entry_point.add_command(db)
entry_point.add_command(evaluate)
entry_point.add_command(analyze)
