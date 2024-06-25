import click
import pandas as pd
from rustyrts_eval.db import history, mutants

from rustyrts_eval.db.analysis import (
    Base,
    history_testcases_different,
    history_testcases_not_contained,
    mutants_failed_not_selected,
    mutants_testcases_contained,
)

from ..db.commands import SUCCESS_CONNECTED_MSG, FAILED_CONNECTED_MSG
from ...db.base import DBConnection
from ...util.logging.cli import (
    click_echo_success,
    click_echo_failure,
    start_spinner,
)
from ...util.logging.logger import get_logger

_LOGGER = get_logger(__name__)

# mutants
SUCCESS_MUTANTS_MSG = "Completed analyzing results of mutants evaluation successfully"
FAILED_MUTANTS_MSG = "Failed analyzing results of mutants evaluation"

# history
SUCCESS_HISTORY_MSG = "Completed analyzing results of git walk successfully"
FAILED_HISTORY_MSG = "Failed analyzing results of git walk"


########################################################################################################################
# Commands


@click.group(name="analyze")
@click.argument("url", type=str)
@click.pass_context
def analyze(ctx, url: str):
    """
    Automatically analyze the results of the evaluation.

    Arguments:

        URL is the database connection string of the format dialect[+driver]://user:password@host/dbname[?key=value..].

    Examples:

        $ rts_eval evaluate postgresql://user:pass@localhost:5432/db mutants
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


@analyze.command(name="mutants")
@click.pass_obj
def mutants_cmd(ctx):
    conn: DBConnection = ctx["connection"]
    try:
        spinner = start_spinner("Analyzing results of mutants evaluation...")

        tables = [
            Base.metadata.tables["MutantsTestsNotContained"],
            Base.metadata.tables["MutantsTestsNotSelected"],
        ]

        _LOGGER.debug("Creating schema with tables: {}".format(Base.metadata.tables.keys()))
        Base.metadata.create_all(conn.engine, tables=tables)

        info = mutants.register_views()
        mutants_failed_not_selected(conn, info)
        mutants_testcases_contained(conn, info)

        spinner.stop()
        click_echo_success(SUCCESS_MUTANTS_MSG)
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_MUTANTS_MSG)
        raise e


@analyze.command(name="history")
@click.pass_obj
def history_cmd(ctx):
    conn: DBConnection = ctx["connection"]
    try:
        spinner = start_spinner("Running git walk testing evaluation...")
        tables = [
            Base.metadata.tables["HistoryTestsNotContained"],
            Base.metadata.tables["HistoryTestsDifferentNotSelected"],
            Base.metadata.tables["HistoryTestsDifferentSelected"],
        ]

        _LOGGER.debug("Creating schema with tables: {}".format(Base.metadata.tables.keys()))
        Base.metadata.create_all(conn.engine, tables=tables)

        info = history.register_views()

        history_testcases_different(conn, info)
        history_testcases_not_contained(conn, info)

        spinner.stop()
        click_echo_success(SUCCESS_HISTORY_MSG)
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_HISTORY_MSG)
        raise e
