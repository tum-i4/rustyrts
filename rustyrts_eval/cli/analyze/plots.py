import click
import pandas as pd
import seaborn as sns
import matplotlib as mpl
import matplotlib.pyplot as plt
from rustyrts_eval.cli.db.commands import FAILED_CONNECTED_MSG, SUCCESS_CONNECTED_MSG
from rustyrts_eval.db import history, mutants
from rustyrts_eval.db.base import DBConnection
from rustyrts_eval.db.mutants import register_views
from rustyrts_eval.db.plots import HistoryPlotter, MutantsPlotter

from rustyrts_eval.util.logging.cli import (
    click_echo_failure,
    click_echo_success,
    start_spinner,
)

from ...util.logging.logger import get_logger

_LOGGER = get_logger(__name__)

output_format = ".pdf"

# mutants
SUCCESS_MUTANTS_MSG = "Completed plotting results of mutants evaluation successfully"
FAILED_MUTANTS_MSG = "Failed plotting results of mutants evaluation"

# history
SUCCESS_HISTORY_MSG = "Completed plotting results of git walk successfully"
FAILED_HISTORY_MSG = "Failed plotting results of git walk"

########################################################################################################################
# Commands


@click.group(name="plot")
@click.argument("url", type=str)
@click.pass_context
def plot(ctx, url: str):
    """
    Plot the results of the evaluation.

    Arguments:

        URL is the database connection string of the format dialect[+driver]://user:password@host/dbname[?key=value..].

    Examples:

        $ rts_eval plot postgresql://user:pass@localhost:5432/db mutants
    """
    # set options
    echo = "debug" if ctx.obj["debug"] else False

    # create db connection
    try:
        spinner = start_spinner("Connecting to database {}".format(url))
        conn = DBConnection(url, echo=echo)
        spinner.stop()
        click_echo_success(SUCCESS_CONNECTED_MSG)
        ctx.obj["connection"] = conn
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_CONNECTED_MSG)
        raise e


@plot.command(name="mutants")
@click.pass_obj
def mutants_cmd(ctx):
    conn: DBConnection = ctx["connection"]
    try:
        spinner = start_spinner("Plotting results of mutants evaluation...")

        info = mutants.register_views()
        plotter = MutantsPlotter(conn, info, output_format)

        partition = True

        plotter.plot_mutants_duration_absolute()
        plotter.plot_mutants_duration_relative()
        plotter.plot_mutants_target_count_absolute(partition=partition)
        plotter.plot_mutants_target_count_relative()
        plotter.plot_mutants_testcases_subsumption()  # (partition=partition)
        plotter.plot_mutants_testcases_count_absolute(partition=partition)
        plotter.plot_mutants_testcases_count_relative()
        plotter.plot_mutants_testcases_failed_absolute(partition=partition)
        plotter.plot_mutants_percentage_failed()

        spinner.stop()
        click_echo_success(SUCCESS_MUTANTS_MSG)
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_MUTANTS_MSG)
        raise e


@plot.command(name="history")
@click.argument("strategy", type=click.Choice(["sequential", "parallel"]), required=True)
@click.pass_obj
def history_cmd(ctx, strategy):
    conn: DBConnection = ctx["connection"]
    try:
        spinner = start_spinner("Plotting git walk testing evaluation...")

        info = history.register_views()
        plotter = HistoryPlotter(
            conn,
            info,
            output_format,
            True if strategy == "sequential" else False,
        )

        partition = True

        plotter.plot_history_duration_absolute(partition=partition)
        plotter.plot_history_duration_relative()
        plotter.plot_history_target_count_absolute(partition=partition)
        plotter.plot_history_target_count_relative()
        plotter.plot_history_testcases_subsumption(partition=partition)
        plotter.plot_history_testcases_count_absolute()
        plotter.plot_history_testcases_count_relative()
        plotter.plot_history_testcases_different_absolute(partition=partition)
        plotter.plot_history_efficiency_repo(partition=partition)

        spinner.stop()
        click_echo_success(SUCCESS_HISTORY_MSG)
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_HISTORY_MSG)
        raise e
