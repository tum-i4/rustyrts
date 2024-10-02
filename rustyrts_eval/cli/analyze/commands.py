import re
import click
from rustyrts_eval.db import history, mutants

from rustyrts_eval.db.history import DBTestReport
from rustyrts_eval.db.mutants import DBMutant

from rustyrts_eval.db.analysis import (
    Base,
    history_testcases_different,
    history_testcases_subsumption,
    mutants_failed_not_selected,
    mutants_testcases_subsumption,
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

# cost
SUCCESS_COST_MSG = "Completed analyzing selection costs"
FAILED_COST_MSG = "Failed analyzing selection costs"


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
            Base.metadata.tables["MutantsSubsumptionStaticSubsumesDynamic"],
            Base.metadata.tables["MutantsSubsumptionBasicSubsumesStatic"],
            Base.metadata.tables["MutantsTestsNotSelected"],
        ]

        _LOGGER.debug("Creating schema with tables: {}".format(Base.metadata.tables.keys()))
        Base.metadata.create_all(conn.engine, tables=tables)

        info = mutants.register_views()
        mutants_failed_not_selected(conn, info)
        mutants_testcases_subsumption(conn, info)

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
            Base.metadata.tables["HistorySubsumptionStaticSubsumesDynamic"],
            Base.metadata.tables["HistorySubsumptionBasicSubsumesStatic"],
            Base.metadata.tables["HistoryTestsDifferentNotSelected"],
            Base.metadata.tables["HistoryTestsDifferentSelected"],
        ]

        _LOGGER.debug("Creating schema with tables: {}".format(Base.metadata.tables.keys()))
        Base.metadata.create_all(conn.engine, tables=tables)

        info = history.register_views()

        history_testcases_different(conn, info)
        history_testcases_subsumption(conn, info)

        spinner.stop()
        click_echo_success(SUCCESS_HISTORY_MSG)
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_HISTORY_MSG)
        raise e


@analyze.command(name="cost")
@click.argument("scheme", type=click.Choice(["history", "mutants"]), required=True)
@click.argument("algorithm", type=click.Choice(["basic", "static", "dynamic"]), required=True)
@click.option("--parent", is_flag=True, help="Consider parent")
@click.pass_obj
def cost(ctx, scheme, algorithm, parent):
    conn: DBConnection = ctx["connection"]
    try:
        spinner = start_spinner("Calculating selection costs...")

        parent_filter = " - parent" if parent else ""
        filter = f"%{algorithm}{parent_filter}"

        with conn.create_session_ctx() as session:
            logs = []

            if scheme == "history":
                db_mutants = sorted(session.query(DBTestReport).where(DBTestReport.__table__.c.name.like(filter)), key=lambda report: report.id)
                for mutant in db_mutants:
                    logs.append((mutant.commit_id, mutant.log))

            if scheme == "mutants":
                db_mutants = sorted(session.query(DBMutant).where(DBTestReport.__table__.c.name.like(filter)), key=lambda report: report.id)
                for mutant in db_mutants:
                    logs.append((mutant.id, mutant.test_log))

            for id, log in logs:
                time = 0.0

                matches = re.finditer(r"RTS.*took (.*)s", log)
                for match in matches:
                    time += float(match.group(1))

                spinner.info(f"{id} took {time:.2f}s to select\n")

        spinner.stop()
        click_echo_success(SUCCESS_COST_MSG)
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_COST_MSG)
        raise e
