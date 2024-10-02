import gc
import click


from sqlalchemy import select
from sqlalchemy.orm import Session

from rustyrts_eval.cli.db.commands import FAILED_CONNECTED_MSG, SUCCESS_CONNECTED_MSG
from rustyrts_eval.db.base import DBConnection
from rustyrts_eval.db.history import DBTestReport, DBTestSuite
from rustyrts_eval.db.mutants import DBMutant, DBMutantsTestSuite
from rustyrts_eval.models.testing.loaders.cargo_test import CargoTestTestReportLoader
from rustyrts_eval.models.testing.mutants import MutantsTestSuite
from rustyrts_eval.util.logging.cli import (
    click_echo_failure,
    click_echo_success,
    start_spinner,
)

from ...util.logging.logger import get_logger

_LOGGER = get_logger(__name__)

output_format = ".pdf"

# mutants
SUCCESS_MUTANTS_MSG = "Completed parsing results of mutants evaluation successfully"
FAILED_MUTANTS_MSG = "Failed parsing results of mutants evaluation"

# history
SUCCESS_HISTORY_MSG = "Completed parsing results of git walk successfully"
FAILED_HISTORY_MSG = "Failed parsing results of git walk"

########################################################################################################################
# Commands


@click.group(name="parse")
@click.argument("url", type=str)
@click.pass_context
def parse(ctx, url: str):
    """
    Parse the results of the evaluation.

    Arguments:

        URL is the database connection string of the format dialect[+driver]://user:password@host/dbname[?key=value..].

    Examples:

        $ rts_eval parse postgresql://user:pass@localhost:5432/db mutants
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


@parse.command(name="mutants")
@click.pass_obj
def mutants_cmd(ctx):
    conn: DBConnection = ctx["connection"]
    try:
        spinner = start_spinner("Consolidating results of mutants evaluation...")

        with conn.create_session_ctx() as session:
            ids_query = select(DBMutant.__table__.c.id).select_from(DBMutant.__table__)
            ids = sorted(session.execute(ids_query).all())
            count_all = len(ids)

            for i, id in enumerate(ids):
                db_mutant = session.query(DBMutant).get(id)
                parse_mutant(session, db_mutant)
                spinner.info("{}/{} done".format(i, count_all))

        spinner.stop()
        click_echo_success(SUCCESS_MUTANTS_MSG)
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_MUTANTS_MSG)
        raise e


@parse.command(name="history")
@click.pass_obj
def history_cmd(ctx):
    conn: DBConnection = ctx["connection"]
    try:
        spinner = start_spinner("Consolidating git walk testing evaluation...")

        with conn.create_session_ctx() as session:
            db_reports = sorted(session.query(DBTestReport).all(), key=lambda report: report.id)
            count_all = len(db_reports)

            for i, db_report in enumerate(db_reports):
                parse_test_report(session, db_report)
                spinner.info("{}/{} done".format(i, count_all))

        spinner.stop()
        click_echo_success(SUCCESS_HISTORY_MSG)
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_HISTORY_MSG)
        raise e


def parse_test_report(sess: Session, report: DBTestReport):
    log = report.log

    test_loader = CargoTestTestReportLoader(log)
    try:
        suites = test_loader.load()
        report.suites = [DBTestSuite.from_domain(suite) for suite in suites]
    except:
        _LOGGER.warning("Failed to parse testsuites of report " + str(report.name) + " on commit " + str(report.commit_id))

    sess.commit()
    gc.collect()


def parse_mutant(sess: Session, mutant: DBMutant):
    test_log = mutant.test_log

    if test_log:
        test_loader = CargoTestTestReportLoader(test_log)
        try:
            suites = [MutantsTestSuite.from_test_suite(suite) for suite in test_loader.load()]
            mutant.suites = [DBMutantsTestSuite.from_domain(suite) for suite in suites]
        except e:
            print(e)
            _LOGGER.warning("Failed to parse testsuites of mutant " + str(mutant.descr))

    sess.commit()
    gc.collect()
