import click
from sqlalchemy import ColumnExpressionArgument, SQLColumnExpression
from sqlalchemy.orm import Session
from rustyrts_eval.cli.db.commands import FAILED_CONNECTED_MSG, SUCCESS_CONNECTED_MSG
from rustyrts_eval.db.base import DBConnection

from typing import Any
from sqlalchemy import update, delete, select, func

from rustyrts_eval.db.git import DBChangelistItem, DBCommit, DBRepository
from rustyrts_eval.db.history import DBTestReport
from rustyrts_eval.db.mutants import DBMutant, DBMutantsReport
from rustyrts_eval.util.logging.cli import (
    click_echo_failure,
    click_echo_success,
    start_spinner,
)

from ...util.logging.logger import get_logger

_LOGGER = get_logger(__name__)

output_format = ".svg"

# mutants
SUCCESS_MUTANTS_MSG = "Completed consolidating results of mutants evaluation successfully"
FAILED_MUTANTS_MSG = "Failed consolidating results of mutants evaluation"

# history
SUCCESS_HISTORY_MSG = "Completed consolidating results of git walk successfully"
FAILED_HISTORY_MSG = "Failed consolidating results of git walk"

########################################################################################################################
# Commands


@click.group(name="consolidate")
@click.argument("url", type=str)
@click.pass_context
def consolidate(ctx, url: str):
    """
    Consolidate the results of the evaluation.

    Arguments:

        URL is the database connection string of the format dialect[+driver]://user:password@host/dbname[?key=value..].

    Examples:

        $ rts_eval consolidate postgresql://user:pass@localhost:5432/db mutants
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


@consolidate.command(name="mutants")
@click.pass_obj
def mutants_cmd(ctx):
    conn: DBConnection = ctx["connection"]
    try:
        spinner = start_spinner("Consolidating results of mutants evaluation...")

        with conn.create_session_ctx() as session:
            ranking_query(session, DBRepository.__table__, [DBRepository.__table__.c.path, DBRepository.__table__.c.id])
            normalizing_query(session, DBRepository.__table__)

            ranking_query(session, DBCommit.__table__, [DBCommit.__table__.c.repo_id, DBCommit.__table__.c.id])
            normalizing_query(session, DBCommit.__table__)

            ranking_query(session, DBChangelistItem.__table__, [DBChangelistItem.__table__.c.commit_id, DBChangelistItem.__table__.c.id])
            normalizing_query(session, DBChangelistItem.__table__)

            ranking_query(session, DBMutantsReport.__table__, [DBMutantsReport.__table__.c.commit_id, DBMutantsReport.__table__.c.id])
            normalizing_query(session, DBMutantsReport.__table__)

            cleanup_query(session, DBMutant.__table__, DBMutant.__table__.c.report_id == None)
            ranking_query(session, DBMutant.__table__, [DBMutant.__table__.c.report_id, DBMutant.__table__.c.id])
            normalizing_query(session, DBMutant.__table__)

            session.commit()

        spinner.stop()
        click_echo_success(SUCCESS_MUTANTS_MSG)
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_MUTANTS_MSG)
        raise e


@consolidate.command(name="history")
@click.pass_obj
def history_cmd(ctx):
    conn: DBConnection = ctx["connection"]
    try:
        spinner = start_spinner("Consolidating git walk testing evaluation...")

        with conn.create_session_ctx() as session:
            ranking_query(session, DBRepository.__table__, [DBRepository.__table__.c.path, DBRepository.__table__.c.id])
            normalizing_query(session, DBRepository.__table__)

            ranking_query(session, DBCommit.__table__, [DBCommit.__table__.c.repo_id, DBCommit.__table__.c.id])
            normalizing_query(session, DBCommit.__table__)

            ranking_query(session, DBChangelistItem.__table__, [DBChangelistItem.__table__.c.commit_id, DBChangelistItem.__table__.c.id])
            normalizing_query(session, DBChangelistItem.__table__)

            ranking_query(session, DBTestReport.__table__, [DBTestReport.__table__.c.commit_id, DBTestReport.__table__.c.id])
            normalizing_query(session, DBTestReport.__table__)

            session.commit()

        spinner.stop()
        click_echo_success(SUCCESS_HISTORY_MSG)
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_HISTORY_MSG)
        raise e


MARGIN = 100000


def ranking_query(sess: Session, table, rank_by: SQLColumnExpression[Any] | list[SQLColumnExpression[Any]]):
    ranks = select(
        table.c.id,
        func.rank().over(order_by=rank_by).label("rank"),
    ).select_from(table)

    query = update(table).values(id=ranks.c.rank + MARGIN).where(ranks.c.id == table.c.id)
    sess.execute(query)


def normalizing_query(sess: Session, table):
    query = update(table).values(id=table.c.id - MARGIN)
    sess.execute(query)


def cleanup_query(sess: Session, table, delete_where: ColumnExpressionArgument):
    query = delete(table).where(delete_where)
    sess.execute(query)
