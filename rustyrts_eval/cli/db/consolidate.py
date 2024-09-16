import re
import click
from sqlalchemy import ColumnExpressionArgument, SQLColumnExpression
from sqlalchemy.orm import Session
from rustyrts_eval.cli.db.commands import FAILED_CONNECTED_MSG, SUCCESS_CONNECTED_MSG
from rustyrts_eval.db.base import DBConnection

from typing import Any, List, Tuple
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

SUCCESS_MSG = "Completed consolidating results of evaluation successfully"
FAILED_MSG = "Failed consolidating results of evaluation"

SUCCESS_DOCTESTS_MSG = "Completed renaming doctests of git walk successfully"
FAILED_DOCTESTS_MSG = "Failed renaming doctests of git walk"

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


@consolidate.command(name="db")
@click.argument("schema", type=click.Choice(["mutants", "history"]), required=True)
@click.pass_obj
def db(ctx, schema):
    conn: DBConnection = ctx["connection"]
    try:
        spinner = start_spinner(f"Consolidating results of {schema} evaluation...")

        with conn.create_session_ctx() as session:
            if schema == "mutants":
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

            if schema == "history":
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
        click_echo_success(SUCCESS_MSG)
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_MSG)
        raise e


@consolidate.command(name="doctests")
@click.pass_obj
def doctests(ctx):
    conn: DBConnection = ctx["connection"]
    try:
        spinner = start_spinner("Renaming doctests")

        with conn.create_session_ctx() as session:
            commits = sorted(session.query(DBCommit).all(), key=lambda commit: commit.id)

            for i, commit in enumerate(commits):
                spinner.info("Processing commit " + str(i))
                shift_doctests(commit)

            session.commit()

        spinner.stop()
        click_echo_success(SUCCESS_DOCTESTS_MSG)
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_DOCTESTS_MSG)
        raise e


def shift_doctests(commit: DBCommit):
    changelist = commit.changelist
    affected = []

    for changelist_item in changelist:
        if changelist_item.filepath.endswith(".rs"):
            positions = extract_positions(changelist_item)

            test_parent, test_actual, basic_parent, basic_actual, static_parent, static_actual, dynamic_parent, dynamic_actual = split_reports(commit.reports)

            if test_parent and test_actual:
                fix_affected(changelist_item.filepath, positions, test_parent, test_actual)
            if basic_parent and basic_actual:
                fix_affected(changelist_item.filepath, positions, basic_parent, basic_actual)
            if static_parent and static_actual:
                fix_affected(changelist_item.filepath, positions, static_parent, static_actual)
            if dynamic_parent and dynamic_actual:
                fix_affected(changelist_item.filepath, positions, dynamic_parent, dynamic_actual)


def fix_affected(file_path, positions, parent: DBTestReport, actual: DBTestReport):
    # somewhat weird exception
    if file_path == "src/compile_fail/must_use.rs":
        return

    affected_tests = []
    for suite in parent.suites:
        for test in suite.cases:
            if test.name.startswith(file_path):
                prefix, line = split_doctest_name(test.name)

                name = test.name
                parent_line = line
                actual_line = line

                acc_added = 0
                acc_removed = 0

                for (removed_start, removed_count), (added_start, added_count) in positions:
                    if parent_line >= removed_start:
                        actual_line -= removed_count
                    if parent_line + (acc_added - acc_removed) >= added_start:
                        actual_line += added_count
                    acc_added += added_count
                    acc_removed += removed_count

                affected_tests.append((prefix, parent_line, actual_line))

    for name, parent_line, actual_line in affected_tests:
        parent_name = name + " (line " + str(parent_line) + ")"
        actual_name = name + " (line " + str(actual_line) + ")"
        new_name = name + " (line " + str(parent_line) + "->" + str(actual_line) + ")"

        for suite in parent.suites:
            for test in suite.cases:
                if test.name.startswith(file_path):
                    if parent_line != actual_line:
                        if test.name == parent_name:
                            # print("(Parent) Renaming " + test.name + " to " + new_name)
                            test.name = new_name

        for suite in actual.suites:
            for test in suite.cases:
                if test.name.startswith(file_path):
                    if parent_line != actual_line:
                        if test.name == actual_name:
                            # print("(Actual) Renaming " + test.name + " to " + new_name)
                            test.name = new_name


def split_reports(reports: [DBTestReport]):
    test_parent = None
    test = None
    basic_parent = None
    basic = None
    static_parent = None
    static = None
    dynamic_parent = None
    dynamic = None

    for report in reports:
        if "test - parent" in report.name and "build" not in report.name:
            test_parent = report
        elif "test" in report.name and "build" not in report.name:
            test = report
        if "basic - parent" in report.name and "build" not in report.name:
            basic_parent = report
        elif "basic" in report.name and "build" not in report.name:
            basic = report
        if "static - parent" in report.name and "build" not in report.name:
            static_parent = report
        elif "static" in report.name and "build" not in report.name:
            static = report
        if "dynamic - parent" in report.name and "build" not in report.name:
            dynamic_parent = report
        elif "dynamic" in report.name and "build" not in report.name:
            dynamic = report

    return (test_parent, test, basic_parent, basic, static_parent, static, dynamic_parent, dynamic)


def split_doctest_name(name: str) -> int:
    parts = name.split(" (line ")
    prefix = parts[0]
    suffix = parts[1]
    line = suffix[:-1]
    return prefix, int(line)


def extract_positions(item: DBChangelistItem) -> List[Tuple[Tuple[int, int], Tuple[int, int]]]:
    positions = []
    matches = re.finditer(r"@@ -(\d+)(,(\d+))? \+(\d+)(,(\d+))? @@", item.content)
    for match in matches:
        groups = match.groups()
        removed_start = int(groups[0])
        removed_count = int(groups[2]) if groups[2] else 1
        added_start = int(groups[3])
        added_count = int(groups[5]) if groups[5] else 1
        positions.append(((removed_start, removed_count), (added_start, added_count)))
    return positions


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
