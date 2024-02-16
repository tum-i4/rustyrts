from sqlalchemy import Enum, String, Integer, Column, select
from sqlalchemy.engine import Engine
from sqlalchemy.ext.declarative import declared_attr, as_declarative
from sqlalchemy.orm import sessionmaker, Session

from rustyrts_eval.db.git import DBCommit
from rustyrts_eval.models.testing.base import TestStatus

from ..util.logging.logger import get_logger

_LOGGER = get_logger(__name__)


def get_test_diff_and_intersection(retest_all, other):
    retest_all_tests = set(retest_all.splitlines()) if retest_all else set()
    other_tests = set(other.splitlines()) if other else set()
    return list(retest_all_tests.difference(other_tests)), list(
        retest_all_tests.intersection(other_tests)
    )


def get_test_diff(retest_all, other):
    retest_all_tests = set(retest_all.splitlines()) if retest_all else set()
    other_tests = set(other.splitlines()) if other else set()
    return list(set(retest_all_tests) - set(other_tests))


@as_declarative()
class Base:
    id = Column(Integer, primary_key=True, index=True)
    reason = Column(String)
    comment = Column(String)
    __name__: str

    @classmethod
    @declared_attr
    def __tablename__(cls) -> str:
        return cls.__name__.removeprefix("DB")


class DBMutantsTestsNotContained(Base):
    commit_id = Column(Integer, nullable=False)
    test_name = Column(String, nullable=False)
    not_contained_count = Column(Integer)

    def __init__(self, commit, test_name, not_contained_count):
        super().__init__()

        self.commit_id = commit
        self.test_name = test_name
        self.not_contained_count = not_contained_count


class DBMutantsTestsNotSelected(Base):
    commit_id = Column(Integer, nullable=False)
    algorithm = Column(String, nullable=False)
    test_name = Column(String, nullable=False)
    not_selected_count = Column(Integer)

    def __init__(self, commit, algorithm, test_name, not_selected_count):
        super().__init__()

        self.commit_id = commit
        self.algorith = algorithm
        self.test_name = test_name
        self.not_selected_count = not_selected_count


def mutants_testcases_contained(connection, view_info):
    labels = view_info.get_labels(connection)

    testcases_selected = view_info.testcases_selected

    selected = (
        select(
            testcases_selected.c.commit.label("repository"),
            testcases_selected.c.retest_all_mutant_id,
            testcases_selected.c.descr.label("mutant"),
            testcases_selected.c.dynamic,
            testcases_selected.c.static,
        )
        .select_from(testcases_selected)
        .where(testcases_selected.c.descr != "baseline")
        .order_by(testcases_selected.c.commit, testcases_selected.c.descr)
    )

    df_selected = connection.query(selected)

    df_selected_dynamic = df_selected[
        ["repository", "retest_all_mutant_id", "mutant", "dynamic"]
    ]
    df_selected_static = df_selected[
        ["repository", "retest_all_mutant_id", "mutant", "static"]
    ]

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

    with connection.create_session_ctx() as session:
        for commit in not_selected_static:
            for test, count in not_selected_static[commit].items():
                entry = DBMutantsTestsNotContained(commit, test, count)
                session.add(entry)
        session.commit()


def mutants_failed_not_selected(connection, view_info):
    labels = view_info.get_labels(connection)

    testcases_selected = view_info.testcases_selected
    testcases_failed = view_info.testcases_failed

    failed_retest_all = (
        select(
            testcases_failed.c.commit.label("repository"),
            testcases_failed.c.retest_all_mutant_id,
            testcases_failed.c.descr.label("mutant"),
            testcases_failed.c.retest_all,
        )
        .select_from(testcases_failed)
        .where(testcases_failed.c.descr != "baseline")
        .order_by(testcases_failed.c.commit, testcases_failed.c.descr)
    )
    selected = (
        select(
            testcases_selected.c.commit.label("repository"),
            testcases_selected.c.retest_all_mutant_id,
            testcases_selected.c.descr.label("mutant"),
            testcases_selected.c.dynamic,
            testcases_selected.c.static,
        )
        .select_from(testcases_selected)
        .where(testcases_selected.c.descr != "baseline")
        .order_by(testcases_selected.c.commit, testcases_selected.c.descr)
    )

    df_failed_retest_all = connection.query(failed_retest_all)
    df_selected_rustyrts = connection.query(selected)

    df_selected_dynamic = df_selected_rustyrts[
        ["repository", "retest_all_mutant_id", "dynamic"]
    ].copy()

    df_selected_static = df_selected_rustyrts[
        ["repository", "retest_all_mutant_id", "static"]
    ].copy()

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
            retest_all_mutant["retest_all"], dynamic_mutant["dynamic"]
        )
        diff_static = get_test_diff(
            retest_all_mutant["retest_all"], static_mutant["static"]
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

    with connection.create_session_ctx() as session:
        for commit in not_selected_dynamic:
            for test, count in not_selected_dynamic[commit].items():
                entry = DBMutantsTestsNotSelected(commit, "dynammic", test, count)
                session.add(entry)
        for commit in not_selected_static:
            for test, count in not_selected_static[commit].items():
                entry = DBMutantsTestsNotSelected(commit, "static", test, count)
                session.add(entry)
        session.commit()


class DBHistoryTestsNotContained(Base):
    repo_id = Column(Integer, nullable=False)
    commit_id = Column(Integer, nullable=False)
    test_name = Column(String, nullable=False)

    def __init__(self, repo_id, commit_id, test_name):
        super().__init__()

        self.repo_id = repo_id
        self.commit_id = commit_id
        self.test_name = test_name


class DBHistoryTestsDifferentNotSelected(Base):
    repo_id = Column(Integer, nullable=False)
    commit_id = Column(Integer, nullable=False)
    algorithm = Column(String, nullable=False)
    test_name = Column(String, nullable=False)
    parent_result = Column(Enum(TestStatus))
    result = Column(Enum(TestStatus))

    def __init__(self, repo_id, commit_id, algorithm, test_name):
        super().__init__()

        self.repo_id = repo_id
        self.commit_id = commit_id
        self.algorithm = algorithm
        self.test_name = test_name


class DBHistoryTestsDifferentSelected(Base):
    repo_id = Column(Integer, nullable=False)
    commit_id = Column(Integer, nullable=False)
    algorithm = Column(String, nullable=False)
    test_name = Column(String, nullable=False)
    parent_result = Column(Enum(TestStatus))
    result = Column(Enum(TestStatus))

    def __init__(self, repo_id, commit_id, algorithm, test_name):
        super().__init__()

        self.repo_id = repo_id
        self.commit_id = commit_id
        self.algorithm = algorithm
        self.test_name = test_name


def history_testcases_not_contained(connection, view_info):
    labels = view_info.get_labels(connection)

    commit = DBCommit.__table__
    testcases_selected = view_info.testcases_selected

    selected = (
        select(
            commit.c.repo_id.label("repository"),
            testcases_selected.c.commit,
            testcases_selected.c.dynamic,
            testcases_selected.c.static,
        )
        .select_from(testcases_selected, commit)
        .where(testcases_selected.c.commit == commit.c.id)
        .order_by(testcases_selected.c.commit)
    )

    df = connection.query(selected)

    df_selected_dynamic = df[["repository", "dynamic", "commit"]]
    df_selected_static = df[["repository", "static", "commit"]]

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

    with connection.create_session_ctx() as session:
        for repository in not_selected_static:
            for commit in not_selected_static[repository]:
                for test in not_selected_static[repository][commit]:
                    entry = DBHistoryTestsNotContained(repository, commit, test)
                    session.add(entry)
        session.commit()


def history_testcases_different(connection, view_info):
    labels = view_info.get_labels(connection)

    commit = DBCommit.__table__
    testcases_selected = view_info.testcases_selected
    testcases_different = view_info.testcases_different

    different_retest_all = (
        select(
            commit.c.repo_id.label("repository"),
            testcases_different.c.commit,
            testcases_different.c.retest_all,
        )
        .select_from(testcases_different, commit)
        .where(commit.c.id == testcases_different.c.commit)
        .order_by(testcases_different.c.commit)
    )
    selected = (
        select(
            commit.c.repo_id.label("repository"),
            testcases_selected.c.commit,
            testcases_selected.c.dynamic,
            testcases_selected.c.static,
        )
        .select_from(testcases_selected, commit)
        .where(commit.c.id == testcases_selected.c.commit)
        .order_by(testcases_selected.c.commit)
    )

    df_different_retest_all = connection.query(different_retest_all)
    df_selected_rustyrts = connection.query(selected)

    df_selected_dynamic = df_selected_rustyrts[
        ["repository", "commit", "dynamic"]
    ].copy()

    df_selected_static = df_selected_rustyrts[["repository", "commit", "static"]].copy()

    not_selected_dynamic = {}
    not_selected_static = {}
    tests_selected_dynamic = {}
    tests_selected_static = {}

    for i in range(1, len(labels) + 1):
        not_selected_dynamic[i] = {}
        not_selected_static[i] = {}
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

        diff_dynamic, intersection_dynamic = get_test_diff_and_intersection(
            retest_all_report["retest_all"], dynamic_report["dynamic"]
        )
        diff_static, intersection_static = get_test_diff_and_intersection(
            retest_all_report["retest_all"], static_report["static"]
        )

        if commit not in not_selected_dynamic[repository]:
            not_selected_dynamic[repository][commit] = []
        if commit not in not_selected_static[repository]:
            not_selected_static[repository][commit] = []

        for test in diff_dynamic:
            not_selected_dynamic[repository][commit].append(test)

        for test in diff_static:
            not_selected_static[repository][commit].append(test)

        if commit not in tests_selected_dynamic[repository]:
            tests_selected_dynamic[repository][commit] = []
        if commit not in tests_selected_static[repository]:
            tests_selected_static[repository][commit] = []

        for test in intersection_dynamic:
            tests_selected_dynamic[repository][commit].append(test)

        for test in intersection_static:
            tests_selected_static[repository][commit].append(test)

    with connection.create_session_ctx() as session:
        for repository in not_selected_static:
            for commit in not_selected_dynamic[repository]:
                for test in not_selected_dynamic[repository][commit]:
                    entry = DBHistoryTestsDifferentNotSelected(
                        repository,
                        commit,
                        "dynamic",
                        test,
                    )
                    session.add(entry)
            for commit in not_selected_static[repository]:
                for test in not_selected_static[repository][commit]:
                    entry = DBHistoryTestsDifferentNotSelected(
                        repository,
                        commit,
                        "static",
                        test,
                    )
                    session.add(entry)
        session.commit()

    with connection.create_session_ctx() as session:
        for repository in tests_selected_static:
            for commit in tests_selected_dynamic[repository]:
                for test in tests_selected_dynamic[repository][commit]:
                    entry = DBHistoryTestsDifferentSelected(
                        repository,
                        commit,
                        "dynamic",
                        test,
                    )
                    session.add(entry)
            for commit in tests_selected_static[repository]:
                for test in tests_selected_static[repository][commit]:
                    entry = DBHistoryTestsDifferentSelected(
                        repository,
                        commit,
                        "static",
                        test,
                    )
                    session.add(entry)
        session.commit()
