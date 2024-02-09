import click
import pandas as pd

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


def get_test_diff(retest_all, other):
    retest_all_tests = retest_all.splitlines()
    other_tests = other.splitlines()
    return list(set(retest_all_tests) - set(other_tests))


ddl_testcases_not_contained = """
create sequence "AnalysisTestsNotContained_id_seq"
    as integer;

alter sequence "AnalysisTestsNotContained_id_seq" owner to postgres;

create table "AnalysisTestsNotContained"
(
    id                 integer default nextval('"AnalysisTestsNotContained_id_seq"'::regclass) not null
        constraint "AnalysisTestsNotContained_pk"
            primary key,
    commit             integer                                                                not null
        constraint "AnalysisTestsNotContained_Commit_null_fk"
            references "Commit",
    test_name          varchar                                                                not null,
    not_contained_count integer                                                                not null,
    reason             varchar,
    comment            varchar
);

alter table "AnalysisTestsNotContained"
    owner to postgres;

"""

ddl_testcases_failed_not_selected = """
create sequence "AnalysisTestsNotSelected_id_seq"
    as integer;

alter sequence "AnalysisTestsNotSelected_id_seq" owner to postgres;

create table "AnalysisTestsNotSelected"
(
    id                 integer default nextval('"AnalysisTestsNotSelected_id_seq"'::regclass) not null
        constraint "AnalysisTestsNotSelected_pk"
            primary key,
    commit             integer                                                                not null
        constraint "AnalysisTestsNotSelected_Commit_null_fk"
            references "Commit",
    test_name          varchar                                                                not null,
    not_selected_count integer                                                                not null,
    reason             varchar,
    comment            varchar,
    algorithm          varchar                                                                not null
);

alter table "AnalysisTestsNotSelected"
    owner to postgres;

"""


def mutants_testcases_contained(conn):
    labels = get_labels_mutants(conn.url)

    df_selected_dynamic = pd.read_sql(
        "SELECT commit as repository, retest_all_mutant_id, dynamic, descr as mutant FROM testcases_selected WHERE descr != 'baseline' ORDER BY commit, descr",
        conn.url,
    )

    df_selected_static = pd.read_sql(
        "SELECT commit as repository, retest_all_mutant_id, static, descr as mutant FROM testcases_selected WHERE descr != 'baseline' ORDER BY commit, descr",
        conn.url,
    )

    not_selected_static = {}

    for i in range(1, len(labels) + 1):
        not_selected_static[i] = {}

    selected_dynamic = df_selected_dynamic.to_dict(orient="records")
    selected_static = df_selected_static.to_dict(orient="records")

    assert len(selected_static) == len(selected_static)

    for dynamic_mutant, static_mutant in zip(selected_dynamic, selected_static):
        assert (
            dynamic_mutant["retest_all_mutant_id"]
            == static_mutant["retest_all_mutant_id"]
        )

        repository = static_mutant["repository"]
        descr = static_mutant["mutant"]

        diff = get_test_diff(dynamic_mutant["dynamic"], static_mutant["static"])

        for test in diff:
            if test not in not_selected_static[repository]:
                not_selected_static[repository][test] = 1
            else:
                not_selected_static[repository][test] += 1

    with conn.engine.connect() as conn:
        conn.execute(ddl_testcases_not_contained)

        for commit in not_selected_static:
            for test, count in not_selected_static[commit].items():
                statement = f"INSERT INTO public.\"AnalysisTestsNotContained\" (commit, test_name, not_contained_count) VALUES ({commit}, '{test}', {count})"
                conn.execute(statement)


def mutants_failed_not_selected(conn):
    labels = get_labels_mutants(conn.url)

    df_failed_retest_all = pd.read_sql(
        "SELECT commit as repository, retest_all_mutant_id, retest_all_failed , descr as mutant FROM testcases_failed WHERE descr != 'baseline' ORDER BY commit, descr",
        conn.url,
    )

    df_selected_dynamic = pd.read_sql(
        "SELECT commit as repository, retest_all_mutant_id, dynamic, descr as mutant FROM testcases_selected WHERE descr != 'baseline' ORDER BY commit, descr",
        conn.url,
    )

    df_selected_static = pd.read_sql(
        "SELECT commit as repository, retest_all_mutant_id, static, descr as mutant FROM testcases_selected WHERE descr != 'baseline' ORDER BY commit, descr",
        conn.url,
    )

    not_selected_dynamic = {}
    not_selected_static = {}

    for i in range(1, len(labels) + 1):
        not_selected_dynamic[i] = {}
        not_selected_static[i] = {}

    failed_retest_all = df_failed_retest_all.to_dict(orient="records")
    selected_dynamic = df_selected_dynamic.to_dict(orient="records")
    selected_static = df_selected_static.to_dict(orient="records")

    assert len(failed_retest_all) == len(selected_dynamic) and len(
        failed_retest_all
    ) == len(selected_static)

    for retest_all_mutant, dynamic_mutant, static_mutant in zip(
        failed_retest_all, selected_dynamic, selected_static
    ):
        assert (
            retest_all_mutant["retest_all_mutant_id"]
            == dynamic_mutant["retest_all_mutant_id"]
        )
        assert (
            retest_all_mutant["retest_all_mutant_id"]
            == static_mutant["retest_all_mutant_id"]
        )

        repository = retest_all_mutant["repository"]
        descr = retest_all_mutant["mutant"]

        diff_dynamic = get_test_diff(
            retest_all_mutant["retest_all_failed"], dynamic_mutant["dynamic"]
        )
        diff_static = get_test_diff(
            retest_all_mutant["retest_all_failed"], static_mutant["static"]
        )

        for test in diff_dynamic:
            if test not in not_selected_dynamic[repository]:
                not_selected_dynamic[repository][test] = 1
            else:
                not_selected_dynamic[repository][test] += 1

        for test in diff_static:
            if test not in not_selected_static[repository]:
                not_selected_static[repository][test] = 1
            else:
                not_selected_static[repository][test] += 1

    with conn.engine.connect() as conn:
        conn.execute(ddl_testcases_not_contained)

        for commit in not_selected_dynamic:
            for test, count in not_selected_dynamic[commit].items():
                statement = f"INSERT INTO public.\"AnalysisTestsNotSelected\" (commit, test_name, not_selected_count, algorithm) VALUES ({commit}, '{test}', {count}, 'dynamic')"
                conn.execute(statement)
        for commit in not_selected_static:
            for test, count in not_selected_static[commit].items():
                statement = f"INSERT INTO public.\"AnalysisTestsNotSelected\" (commit, test_name, not_selected_count, algorithm) VALUES ({commit}, '{test}', {count}, 'static')"
                conn.execute(statement)


ddl_history_testcases_not_contained = """
create sequence "AnalysisTestsNotContained_id_seq"
    as integer;

alter sequence "AnalysisTestsNotContained_id_seq" owner to postgres;

create table "AnalysisTestsNotContained"
(
    id                 integer default nextval('"AnalysisTestsNotContained_id_seq"'::regclass) not null
        constraint "AnalysisTestsNotContained_pk"
            primary key,
    repo_id            integer                                                                not null
        constraint "AnalysisTestsNotContained_Repoistory_null_fk"
            references "Repository",
    commit             integer                                                                not null
        constraint "AnalysisTestsNotContained_Commit_null_fk"
            references "Commit",
    test_name          varchar                                                                not null,
    reason             varchar,
    comment            varchar
);

alter table "AnalysisTestsNotContained"
    owner to postgres;

"""


def history_testcases_not_contained(conn):
    labels = get_labels_git(conn.url)

    df_selected_dynamic = pd.read_sql(
        'SELECT c.repo_id as repository, commit, dynamic FROM testcases_selected join "Commit" c ON c.id = commit ORDER BY commit',
        conn.url,
    )

    df_selected_static = pd.read_sql(
        'SELECT c.repo_id as repository, commit, static FROM testcases_selected join "Commit" c ON c.id = commit ORDER BY commit',
        conn.url,
    )

    not_selected_static = {}

    for i in range(1, len(labels) + 1):
        not_selected_static[i] = {}

    selected_dynamic = df_selected_dynamic.to_dict(orient="records")
    selected_static = df_selected_static.to_dict(orient="records")

    assert len(selected_static) == len(selected_static)

    for dynamic_report, static_report in zip(selected_dynamic, selected_static):
        assert dynamic_report["commit"] == static_report["commit"]

        repository = static_report["repository"]
        commit = static_report["commit"]

        diff = get_test_diff(dynamic_report["dynamic"], static_report["static"])

        if commit not in not_selected_static[repository]:
            not_selected_static[repository][commit] = []

        for test in diff:
            not_selected_static[repository][commit].append(test)

    with conn.engine.connect() as conn:
        conn.execute(ddl_history_testcases_not_contained)

        for repository in not_selected_static:
            for commit in not_selected_static[repository]:
                for test in not_selected_static[repository][commit]:
                    statement = f"INSERT INTO public.\"AnalysisTestsNotContained\" (repo_id, commit, test_name) VALUES ({repository}, {commit}, '{test}')"
                    conn.execute(statement)


ddl_history_testcases_different_not_selected = """
create sequence "AnalysisDifferentNotSelected_id_seq"
    as integer;

alter sequence "AnalysisDifferentNotSelected_id_seq" owner to postgres;

create table "AnalysisDifferentNotSelected"
(
    id                 integer default nextval('"AnalysisDifferentNotSelected_id_seq"'::regclass) not null
        constraint "AnalysisDifferentNotSelected_pk"
            primary key,
    repo_id            integer                                                                not null
        constraint "AnalysisTestsNotContained_Repoistory_null_fk"
            references "Repository",
    commit             integer                                                                not null
        constraint "AnalysisDifferentNotSelected_Commit_null_fk"
            references "Commit",
    test_name          varchar                                                                not null,
    parent_result      teststatus,
    result         teststatus,
    reason             varchar,
    comment            varchar,
    algorithm          varchar                                                                not null
);

alter table "AnalysisDifferentNotSelected"
    owner to postgres;

"""


def history_testcases_different_not_selected(conn):
    labels = get_labels_git(conn.url)

    def get_test_diff(retest_all, other):
        retest_all_tests = retest_all.splitlines()
        other_tests = other.splitlines()
        return list(set(retest_all_tests) - set(other_tests))

    df_different_retest_all = pd.read_sql(
        'SELECT c.repo_id as repository, commit, retest_all_different FROM testcases_newly_different join "Commit" c ON c.id = commit ORDER BY commit',
        conn.url,
    )

    df_selected_dynamic = pd.read_sql(
        'SELECT c.repo_id as repository, commit, dynamic FROM testcases_selected join "Commit" c ON c.id = commit ORDER BY commit',
        conn.url,
    )

    df_selected_static = pd.read_sql(
        'SELECT c.repo_id as repository, commit, static FROM testcases_selected join "Commit" c ON c.id = commit ORDER BY commit',
        conn.url,
    )

    not_selected_dynamic = {}
    not_selected_static = {}

    for i in range(1, len(labels) + 1):
        not_selected_dynamic[i] = {}
        not_selected_static[i] = {}

    different_retest_all = df_different_retest_all.to_dict(orient="records")
    selected_dynamic = df_selected_dynamic.to_dict(orient="records")
    selected_static = df_selected_static.to_dict(orient="records")

    assert len(different_retest_all) == len(selected_dynamic) and len(
        different_retest_all
    ) == len(selected_static)

    for retest_all_report, dynamic_report, static_report in zip(
        different_retest_all, selected_dynamic, selected_static
    ):
        assert retest_all_report["commit"] == dynamic_report["commit"]
        assert retest_all_report["commit"] == static_report["commit"]

        repository = retest_all_report["repository"]
        commit = retest_all_report["commit"]

        diff_dynamic = get_test_diff(
            retest_all_report["retest_all_different"], dynamic_report["dynamic"]
        )
        diff_static = get_test_diff(
            retest_all_report["retest_all_different"], static_report["static"]
        )

        if commit not in not_selected_dynamic[repository]:
            not_selected_dynamic[repository][commit] = []
        if commit not in not_selected_static[repository]:
            not_selected_static[repository][commit] = []

        for test in diff_dynamic:
            not_selected_dynamic[repository][commit].append(test)

        for test in diff_static:
            not_selected_static[repository][commit].append(test)

    with conn.engine.connect() as conn:
        conn.execute(ddl_history_testcases_different_not_selected)

        for repository in not_selected_static:
            for commit in not_selected_dynamic[repository]:
                for test in not_selected_dynamic[repository][commit]:
                    statement = f"INSERT INTO public.\"AnalysisDifferentNotSelected\" (repo_id, commit, test_name, algorithm) VALUES ({repository}, {commit}, '{test}', 'dynamic')"
                    conn.execute(statement)
            for commit in not_selected_static[repository]:
                for test in not_selected_static[repository][commit]:
                    statement = f"INSERT INTO public.\"AnalysisDifferentNotSelected\" (repo_id, commit, test_name, algorithm) VALUES ({repository}, {commit}, '{test}', 'static')"
                    conn.execute(statement)


table_ddl = """
create sequence "AnalysisDifferentSelected_id_seq"
    as integer;

alter sequence "AnalysisDifferentSelected_id_seq" owner to postgres;

create table "AnalysisDifferentSelected"
(
    id                 integer default nextval('"AnalysisDifferentSelected_id_seq"'::regclass) not null
        constraint "AnalysisDifferentSelected_pk"
            primary key,
    repo_id            integer                                                                not null
        constraint "AnalysisTestsNotContained_Repoistory_null_fk"
            references "Repository",
    commit             integer                                                                not null
        constraint "AnalysisDifferentSelected_Commit_null_fk"
            references "Commit",
    test_name          varchar                                                                not null,
    parent_result      teststatus,
    result         teststatus,
    reason             varchar,
    comment            varchar,
    algorithm          varchar                                                                not null
);

alter table "AnalysisDifferentSelected"
    owner to postgres;

"""


def history_testcases_different_selected(conn):
    labels = get_labels_git(conn.url)

    def get_test_intersection(retest_all, other):
        retest_all_tests = set(retest_all.splitlines())
        other_tests = set(other.splitlines())
        return list(retest_all_tests.intersection(other_tests))

    df_different_retest_all = pd.read_sql(
        'SELECT c.repo_id as repository, commit, retest_all_different FROM testcases_newly_different join "Commit" c ON c.id = commit ORDER BY commit',
        conn.url,
    )

    df_selected_dynamic = pd.read_sql(
        'SELECT c.repo_id as repository, commit, dynamic FROM testcases_selected join "Commit" c ON c.id = commit ORDER BY commit',
        conn.url,
    )

    df_selected_static = pd.read_sql(
        'SELECT c.repo_id as repository, commit, static FROM testcases_selected join "Commit" c ON c.id = commit ORDER BY commit',
        conn.url,
    )

    tests_selected_dynamic = {}
    tests_selected_static = {}

    for i in range(1, len(labels) + 1):
        tests_selected_dynamic[i] = {}
        tests_selected_static[i] = {}

    different_retest_all = df_different_retest_all.to_dict(orient="records")
    selected_dynamic = df_selected_dynamic.to_dict(orient="records")
    selected_static = df_selected_static.to_dict(orient="records")

    assert len(different_retest_all) == len(selected_dynamic) and len(
        different_retest_all
    ) == len(selected_static)

    for retest_all_report, dynamic_report, static_report in zip(
        different_retest_all, selected_dynamic, selected_static
    ):
        assert retest_all_report["commit"] == dynamic_report["commit"]
        assert retest_all_report["commit"] == static_report["commit"]

        repository = retest_all_report["repository"]
        commit = retest_all_report["commit"]

        diff_dynamic = get_test_intersection(
            retest_all_report["retest_all_different"], dynamic_report["dynamic"]
        )
        diff_static = get_test_intersection(
            retest_all_report["retest_all_different"], static_report["static"]
        )

        if commit not in tests_selected_dynamic[repository]:
            tests_selected_dynamic[repository][commit] = []
        if commit not in tests_selected_static[repository]:
            tests_selected_static[repository][commit] = []

        for test in diff_dynamic:
            tests_selected_dynamic[repository][commit].append(test)

        for test in diff_static:
            tests_selected_static[repository][commit].append(test)

    with conn.engine.connect() as conn:
        conn.execute(table_ddl)

        for repository in tests_selected_static:
            for commit in tests_selected_dynamic[repository]:
                for test in tests_selected_dynamic[repository][commit]:
                    statement = f"INSERT INTO public.\"AnalysisDifferentSelected\" (repo_id, commit, test_name, algorithm) VALUES ({repository}, {commit}, '{test}', 'dynamic')"
                    conn.execute(statement)
            for commit in tests_selected_static[repository]:
                for test in tests_selected_static[repository][commit]:
                    statement = f"INSERT INTO public.\"AnalysisDifferentSelected\" (repo_id, commit, test_name, algorithm) VALUES ({repository}, {commit}, '{test}', 'static')"
                    conn.execute(statement)


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
def mutants(ctx):
    conn: DBConnection = ctx["connection"]
    try:
        spinner = start_spinner("Analyzing results of mutants evaluation...")

        mutants_failed_not_selected(conn)
        mutants_testcases_contained(conn)

        spinner.stop()
        click_echo_success(SUCCESS_MUTANTS_MSG)
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_MUTANTS_MSG)
        raise e


@analyze.command(name="history")
@click.pass_obj
def history(ctx):
    conn: DBConnection = ctx["connection"]
    try:
        spinner = start_spinner("Running git walk testing evaluation...")

        history_testcases_different_not_selected(conn)
        history_testcases_different_selected(conn)
        history_testcases_not_contained(conn)

        spinner.stop()
        click_echo_success(SUCCESS_HISTORY_MSG)
    except Exception as e:
        _LOGGER.debug(e)
        click_echo_failure(FAILED_HISTORY_MSG)
        raise e
