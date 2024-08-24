from decimal import ROUND_DOWN
from typing import List, Optional
from numpy.lib.function_base import average

from sqlalchemy import (
    NUMERIC,
    Column,
    String,
    Integer,
    ForeignKey,
    Float,
    Enum,
    Text,
    UniqueConstraint,
    Boolean,
    Index,
    cast,
    distinct,
    literal_column,
    select,
)

from sqlalchemy.sql import exists, func
from sqlalchemy.orm import Mapped, relationship, Session
from sqlalchemy.sql.functions import aggregate_strings, coalesce, count, sum
from sqlalchemy_utils import create_materialized_view, create_view

from .base import Base
from .git import DBCommit, DBRepository
from ..models.testing.base import (
    TestReport,
    TestSuite,
    TestCase,
    TestStatus,
    TestTarget,
)


########################################################################################################################
# Meta classes
#


class DBTestReportMeta(Base.__class__, TestReport.__class__): ...


class DBTestSuiteMeta(Base.__class__, TestSuite.__class__): ...


class DBTestCaseMeta(Base.__class__, TestCase.__class__): ...


########################################################################################################################
# Actual db classes
#


class DBTestReport(Base, TestReport, metaclass=DBTestReportMeta):
    __table_args__ = (
        Index("ix_TestReport_name", "name"),
        Index("ix_TestsReport_commit", "commit_str"),
    )

    name = Column(String, nullable=False)
    duration = Column(Float)
    build_duration = Column(Float)
    suites: Mapped[List["DBTestSuite"]] = relationship("DBTestSuite", back_populates="report")
    commit_str = Column(String, nullable=False)
    commit_id = Column(Integer, ForeignKey("{}.id".format(DBCommit.__tablename__), ondelete="CASCADE", onupdate="CASCADE"))
    commit: Mapped[DBCommit] = relationship("DBCommit", back_populates="reports")
    log = Column(Text)
    has_failed = Column(Boolean)
    has_errored = Column(Boolean)

    __table_args__ = tuple([UniqueConstraint("name", "commit_str", name="_test_name_revision_uc")])

    @classmethod
    def get_single(cls, name: str, commit_str: str, session: Session) -> Optional["DBTestReport"]:
        db_report: Optional[DBTestReport] = session.query(DBTestReport).filter_by(name=name, commit_str=commit_str).first()
        return db_report

    @classmethod
    def create_or_update(cls, report: TestReport, session: Session) -> "DBTestReport":
        # get report from DB
        db_report: Optional[DBTestReport] = cls.get_single(name=report.name, commit_str=report.commit_str, session=session)

        # create DB report object if not in DB yet
        if not db_report:
            # get commits if exists, otherwise create
            if report.commit:
                report.commit = DBCommit.create_or_get(report.commit, session)

            db_report = DBTestReport.from_domain(report)
            session.add(db_report)
        else:
            # if already existing, update all fields
            db_report.duration = report.duration if report.duration else db_report.duration
            db_report.build_duration = report.build_duration if report.build_duration else db_report.build_duration
            db_report.commit_str = report.commit_str if report.commit_str else db_report.commit_str
            # get from db if it exists
            db_report.commit = DBCommit.create_or_get(report.commit, session)
            db_report.suites = [DBTestSuite.from_domain(s) for s in report.suites] if report.suites is not None else db_report.suites
            db_report.log = report.log if report.log else db_report.log
            db_report.has_failed = report.has_failed if report.has_failed is not None else db_report.has_failed
            db_report.has_errored = report.has_errored if report.has_errored is not None else db_report.has_errored
        return db_report

    @classmethod
    def from_domain(cls, report: TestReport) -> "DBTestReport":
        if isinstance(report, cls) or not report:
            return report
        return cls(
            name=report.name,
            duration=report.duration,
            build_duration=report.build_duration,
            suites=[] if report.suites is None else [DBTestSuite.from_domain(suite) for suite in report.suites],
            commit_str=report.commit_str,
            commit=DBCommit.from_domain(report.commit),
            log=report.log,
            has_failed=report.has_failed,
            has_errored=report.has_errored,
        )

    def to_domain(self) -> TestReport:
        return TestReport(
            name=self.name,
            duration=self.duration,
            build_duration=self.build_duration,
            suites=[DBTestSuite.to_domain(suite) for suite in self.suites],
            commit_str=self.commit_str,
            commit=self.commit.to_domain(),
            log=self.log,
            has_failed=self.has_failed,
            has_errored=self.has_errored,
        )


class DBTestSuite(Base, TestSuite, metaclass=DBTestSuiteMeta):
    __table_args__ = (
        Index("ix_TestSuite_id_report_id_name", "id", "report_id", "name"),
        Index("ix_TestSuite_name", "name"),
        Index("ix_TestSuite_crashed", "crashed"),
    )

    name = Column(String, nullable=False)
    duration = Column(Float)
    crashed = Column(Boolean)
    total_count = Column(Integer)
    passed_count = Column(Integer)
    failed_count = Column(Integer)
    ignored_count = Column(Integer)
    measured_count = Column(Integer)
    filtered_out_count = Column(Integer)
    report_id = Column(
        Integer,
        ForeignKey("{}.id".format(DBTestReport.__tablename__), ondelete="CASCADE", onupdate="CASCADE"),
    )
    report = relationship("DBTestReport", back_populates="suites")
    cases: Mapped[List["DBTestCase"]] = relationship("DBTestCase", back_populates="suite", cascade="all, delete-orphan")

    @classmethod
    def from_domain(cls, suite: TestSuite) -> "DBTestSuite":
        if isinstance(suite, cls) or not suite:
            return suite
        return cls(
            name=suite.name,
            duration=suite.duration,
            crashed=suite.crashed,
            cases=[DBTestCase.from_domain(case) for case in suite.cases],
            total_count=suite.total_count,
            passed_count=suite.passed_count,
            failed_count=suite.failed_count,
            ignored_count=suite.ignored_count,
            measured_count=suite.measured_count,
            filtered_out_count=suite.filtered_out_count,
        )

    def to_domain(self) -> TestSuite:
        return TestSuite(
            name=self.name,
            duration=self.duration,
            crashed=self.crashed,
            cases=[c.to_domain() for c in self.cases],
            total_count=self.total_count,
            passed_count=self.passed_count,
            failed_count=self.failed_count,
            ignored_count=self.ignored_count,
            measured_count=self.measured_count,
            filtered_out_count=self.filtered_out_count,
        )


class DBTestCase(Base, TestCase, metaclass=DBTestCaseMeta):
    __table_args__ = (
        Index("ix_TestCase_id_suite_id_status", "id", "suite_id", "status"),
        Index("ix_TestCase_name", "name"),
        Index("ix_TestCase_status", "status"),
    )

    name = Column(String, nullable=True)
    target = Column(Enum(TestTarget))
    status = Column(Enum(TestStatus))
    duration = Column(Float)
    suite_id = Column(
        Integer,
        ForeignKey("{}.id".format(DBTestSuite.__tablename__), ondelete="CASCADE", onupdate="CASCADE"),
    )
    suite = relationship("DBTestSuite", back_populates="cases")
    stdout = Column(String)

    @classmethod
    def from_domain(cls, case: TestCase) -> "DBTestCase":
        if isinstance(case, cls) or not case:
            return case
        return cls(
            name=case.name,
            target=case.target,
            status=case.status,
            duration=case.duration,
            stdout=case.stdout,
        )

    def to_domain(self) -> TestCase:
        return TestCase(
            name=self.name,
            target=self.target,
            status=self.status,
            duration=self.duration,
            stdout=self.stdout,
        )


########################################################################################################################
# Views


class HistoryViewInformation:
    def __init__(
        self,
        overview,
        duration,
        testreport_extended,
        target_count,
        testcases_count,
        testcases_different,
        testcases_selected,
        statistics_commit,
        statistics_repository,
    ):
        super().__init__()
        self.overview = overview
        self.duration = duration
        self.testreport_extended = testreport_extended
        self.target_count = target_count
        self.testcases_count = testcases_count
        self.testcases_different = testcases_different
        self.testcases_selected = testcases_selected
        self.statistics_commit = statistics_commit
        self.statistics_repository = statistics_repository

    def get_labels(self, connection):
        repository = DBRepository.__table__
        commit = DBCommit.__table__
        overview = self.overview

        labels_history = (
            select(
                repository.c.id,
                repository.c.path.concat(literal_column("'\n('").concat(count(distinct(overview.c.commit)).label("number_commits").concat(literal_column("')'")))).label("path"),
            )
            .select_from(repository, commit, overview)
            .where(repository.c.id == commit.c.repo_id)
            .where(commit.c.id == overview.c.commit)
            .group_by(repository.c.id, repository.c.path)
            .order_by(repository.c.path)
        )

        df_labels = connection.query(labels_history)
        df_labels["path"] = df_labels["path"].apply(lambda x: x[x.rfind("/") + 1 :])
        return df_labels


def register_views_individual(special):
    commit = DBCommit.__table__
    report = DBTestReport.__table__

    retest_all = report.alias("retest_all")
    basic = report.alias("basic")
    dynamic = report.alias("dynamic")
    static = report.alias("static")

    testreport_extended = (
        select(
            commit.c.id.label("commit"),
            commit.c.commit_str,
            commit.c.repo_id,
            retest_all.c.id.label("retest_all_id"),
            retest_all.c.log.label("retest_all_test_log"),
            retest_all.c.duration.label("retest_all_test_duration"),
            retest_all.c.build_duration.label("retest_all_build_duration"),
            basic.c.id.label("basic_id"),
            basic.c.log.label("basic_test_log"),
            basic.c.duration.label("basic_test_duration"),
            basic.c.build_duration.label("basic_build_duration"),
            dynamic.c.id.label("dynamic_id"),
            dynamic.c.log.label("dynamic_test_log"),
            dynamic.c.duration.label("dynamic_test_duration"),
            dynamic.c.build_duration.label("dynamic_build_duration"),
            static.c.id.label("static_id"),
            static.c.log.label("static_test_log"),
            static.c.duration.label("static_test_duration"),
            static.c.build_duration.label("static_build_duration"),
        )
        .select_from(
            commit,
            retest_all,
            basic,
            dynamic,
            static,
        )
        .where(commit.c.id == retest_all.c.commit_id)
        .where(commit.c.id == basic.c.commit_id)
        .where(commit.c.id == dynamic.c.commit_id)
        .where(commit.c.id == static.c.commit_id)
        .where(retest_all.c.name == f"cargo test{special}")
        .where(basic.c.name == f"cargo rustyrts basic{special}")
        .where(dynamic.c.name == f"cargo rustyrts dynamic{special}")
        .where(static.c.name == f"cargo rustyrts static{special}")
        .where(retest_all.c.has_errored == False)
        .where(basic.c.has_errored == False)
        .where(dynamic.c.has_errored == False)
        .where(static.c.has_errored == False)
    )

    testreport_parent_extended = (
        select(
            commit.c.id.label("commit"),
            commit.c.commit_str,
            commit.c.repo_id,
            retest_all.c.id.label("retest_all_id"),
            retest_all.c.log.label("retest_all_test_log"),
            retest_all.c.duration.label("retest_all_test_duration"),
            retest_all.c.build_duration.label("retest_all_build_duration"),
            basic.c.id.label("basic_id"),
            basic.c.log.label("basic_test_log"),
            basic.c.duration.label("basic_test_duration"),
            basic.c.build_duration.label("basic_build_duration"),
            dynamic.c.id.label("dynamic_id"),
            dynamic.c.log.label("dynamic_test_log"),
            dynamic.c.duration.label("dynamic_test_duration"),
            dynamic.c.build_duration.label("dynamic_build_duration"),
            static.c.id.label("static_id"),
            static.c.log.label("static_test_log"),
            static.c.duration.label("static_test_duration"),
            static.c.build_duration.label("static_build_duration"),
        )
        .select_from(
            commit,
            retest_all,
            basic,
            dynamic,
            static,
        )
        .where(commit.c.id == retest_all.c.commit_id)
        .where(commit.c.id == basic.c.commit_id)
        .where(commit.c.id == dynamic.c.commit_id)
        .where(commit.c.id == static.c.commit_id)
        .where(retest_all.c.name == f"cargo test{special} - parent")
        .where(basic.c.name == f"cargo rustyrts basic{special} - parent")
        .where(dynamic.c.name == f"cargo rustyrts dynamic{special} - parent")
        .where(static.c.name == f"cargo rustyrts static{special} - parent")
        .where(retest_all.c.has_errored == False)
        .where(dynamic.c.has_errored == False)
        .where(static.c.has_errored == False)
    )

    return testreport_extended.cte(), testreport_parent_extended.cte()


def register_views(sequential: bool = False) -> HistoryViewInformation:
    commit = DBCommit.__table__
    repository = DBRepository.__table__

    report = DBTestReport.__table__
    suite = DBTestSuite.__table__
    case = DBTestCase.__table__

    special = " sequentially" if sequential else ""

    check_parsed_tests = (
        select(
            suite,
            count(case.c.id).label("count_cases"),
            (suite.c.passed_count + suite.c.failed_count + suite.c.measured_count - count(case.c.id)).label("discrepancy"),
        )
        .select_from(suite, case)
        .where(suite.c.id == case.c.suite_id)
        .where(case.c.status != "IGNORED")
        .group_by(suite)
        .having(count(case.c.id) != (suite.c.passed_count + suite.c.failed_count + suite.c.measured_count))
    )

    check_parsed_tests = create_view("CheckParsedTests", check_parsed_tests, replace=True, metadata=Base.metadata)

    statistics_commit = (
        select(
            commit.c.id,
            commit.c.repo_id,
            commit.c.commit_str,
            commit.c.nr_lines.label("lines"),
            commit.c.nr_files.label("files"),
            count(distinct(suite.c.id)).label("suites"),
            sum(select(count(distinct(case.c.id))).select_from(case).where(case.c.suite_id == suite.c.id).where(case.c.status != "IGNORED").scalar_subquery()).label("cases"),
            sum(select(count(distinct(case.c.id))).select_from(case).where(case.c.suite_id == suite.c.id).where(case.c.target == "UNIT").where(case.c.status != "IGNORED").scalar_subquery()).label("unit"),
            sum(select(count(distinct(case.c.id))).select_from(case).where(case.c.suite_id == suite.c.id).where(case.c.target == "INTEGRATION").where(case.c.status != "IGNORED").scalar_subquery()).label("integration"),
            func.round(func.cast(sum(suite.c.duration), NUMERIC), 2).label("test_duration"),
        )
        .select_from(commit, report, suite)
        .where(commit.c.id == report.c.commit_id)
        .where(report.c.id == suite.c.report_id)
        .where(report.c.has_errored == False)
        .where(report.c.name == "cargo test" + special)
        .group_by(commit.c.id, commit.c.repo_id)
    )

    statistics_commit = create_materialized_view(
        "StatisticsCommit",
        statistics_commit,
        # replace=True,
        metadata=Base.metadata,
    )

    statistics_repository = (
        select(
            statistics_commit.c.repo_id,
            func.round(
                func.cast(func.avg(statistics_commit.c.lines), NUMERIC),
                2,
            ).label("avg_lines"),
            func.round(
                func.cast(func.avg(statistics_commit.c.files), NUMERIC),
                2,
            ).label("avg_files"),
            func.round(
                func.cast(func.avg(statistics_commit.c.suites), NUMERIC),
                2,
            ).label("avg_suites"),
            func.round(
                func.cast(func.avg(statistics_commit.c.cases), NUMERIC),
                2,
            ).label("avg_cases"),
            func.round(
                func.cast(func.avg(statistics_commit.c.unit), NUMERIC),
                2,
            ).label("avg_unit"),
            func.round(
                func.cast(func.avg(statistics_commit.c.integration), NUMERIC),
                2,
            ).label("avg_integration"),
            func.round(
                func.cast(func.avg(statistics_commit.c.test_duration), NUMERIC),
                2,
            ).label("avg_test_duration"),
        )
        .select_from(statistics_commit)
        .group_by(statistics_commit.c.repo_id)
    )

    statistics_repository = create_materialized_view(
        "StatisticsRepository",
        statistics_repository,
        # replace=True,
        metadata=Base.metadata,
    )

    report, report_parent = register_views_individual(special)

    report = create_view(
        "TestReportExtended",
        report,
        replace=True,
        metadata=Base.metadata,
    )
    report_parent = create_view(
        "TestReportParentExtended",
        report_parent,
        replace=True,
        metadata=Base.metadata,
    )

    retest_all = report.alias("retest_all")
    basic = report.alias("basic")
    dynamic = report.alias("dynamic")
    static = report.alias("static")

    testcase_extended = (
        select(
            case,
            suite.c.crashed,
            suite.c.name.label("testsuite_name"),
            suite.c.report_id,
        )
        .select_from(suite, case)
        .where(suite.c.id == case.c.suite_id)
        .where(case.c.status != "IGNORED")
    )

    testcase = testcase_extended.cte()

    retest_all_testcases = testcase.alias("retest_all_test_cases")
    basic_testcases = testcase.alias("basic_test_cases")
    dynamic_testcases = testcase.alias("dynamic_test_cases")
    static_testcases = testcase.alias("static_test_cases")

    overview = (
        select(
            report.c.commit,
            report.c.retest_all_id.label("retest_all_id"),
            report.c.basic_id.label("basic_id"),
            report.c.dynamic_id.label("dynamic_id"),
            report.c.static_id.label("static_id"),
            retest_all_testcases.c.target.label("target"),
            retest_all_testcases.c.testsuite_name.label("retest_all_suite_name"),
            retest_all_testcases.c.name.label("retest_all_name"),
            retest_all_testcases.c.id.label("retest_all_testcase_id"),
            retest_all_testcases.c.status.label("retest_all_status"),
            basic_testcases.c.testsuite_name.label("basic_suite_name"),
            basic_testcases.c.name.label("basic_name"),
            basic_testcases.c.id.label("basic_testcase_id"),
            basic_testcases.c.status.label("basic_status"),
            dynamic_testcases.c.testsuite_name.label("dynamic_suite_name"),
            dynamic_testcases.c.name.label("dynamic_name"),
            dynamic_testcases.c.id.label("dynamic_testcase_id"),
            dynamic_testcases.c.status.label("dynamic_status"),
            static_testcases.c.testsuite_name.label("static_suite_name"),
            static_testcases.c.name.label("static_name"),
            static_testcases.c.id.label("static_testcase_id"),
            static_testcases.c.status.label("static_status"),
        )
        .select_from(report)
        .join(
            retest_all_testcases,
            report.c.retest_all_id == retest_all_testcases.c.report_id,
        )
        .outerjoin(
            basic_testcases,
            (report.c.basic_id == basic_testcases.c.report_id) & (retest_all_testcases.c.name == basic_testcases.c.name) & (retest_all_testcases.c.testsuite_name == basic_testcases.c.testsuite_name),
        )
        .outerjoin(
            dynamic_testcases,
            (report.c.dynamic_id == dynamic_testcases.c.report_id) & (retest_all_testcases.c.name == dynamic_testcases.c.name) & (retest_all_testcases.c.testsuite_name == dynamic_testcases.c.testsuite_name),
        )
        .outerjoin(
            static_testcases,
            (report.c.static_id == static_testcases.c.report_id) & (retest_all_testcases.c.name == static_testcases.c.name) & (retest_all_testcases.c.testsuite_name == static_testcases.c.testsuite_name),
        )
        .where(retest_all_testcases.c.crashed == False)  # filter suites that are not comparable
        .where((basic_testcases.c.crashed == None) | (basic_testcases.c.crashed == False))
        .where((dynamic_testcases.c.crashed == None) | (dynamic_testcases.c.crashed == False))
        .where((static_testcases.c.crashed == None) | (static_testcases.c.crashed == False))
        .where(retest_all_testcases.c.status != "IGNORED")
    )

    overview = create_materialized_view(
        "TestCaseOverview",
        overview,
        # replace=True,
        metadata=Base.metadata,
    )

    overview_parent = (
        select(
            report_parent.c.commit,
            report_parent.c.retest_all_id.label("retest_all_id"),
            report_parent.c.basic_id.label("basic_id"),
            report_parent.c.dynamic_id.label("dynamic_id"),
            report_parent.c.static_id.label("static_id"),
            retest_all_testcases.c.target.label("target"),
            retest_all_testcases.c.testsuite_name.label("retest_all_suite_name"),
            retest_all_testcases.c.name.label("retest_all_name"),
            retest_all_testcases.c.id.label("retest_all_testcase_id"),
            retest_all_testcases.c.status.label("retest_all_status"),
            basic_testcases.c.testsuite_name.label("basic_suite_name"),
            basic_testcases.c.name.label("basic_name"),
            basic_testcases.c.id.label("basic_testcase_id"),
            basic_testcases.c.status.label("basic_status"),
            dynamic_testcases.c.testsuite_name.label("dynamic_suite_name"),
            dynamic_testcases.c.name.label("dynamic_name"),
            dynamic_testcases.c.id.label("dynamic_testcase_id"),
            dynamic_testcases.c.status.label("dynamic_status"),
            static_testcases.c.testsuite_name.label("static_suite_name"),
            static_testcases.c.name.label("static_name"),
            static_testcases.c.id.label("static_testcase_id"),
            static_testcases.c.status.label("static_status"),
        )
        .select_from(report_parent)
        .join(
            retest_all_testcases,
            report_parent.c.retest_all_id == retest_all_testcases.c.report_id,
        )
        .outerjoin(
            basic_testcases,
            (report_parent.c.basic_id == basic_testcases.c.report_id) & (retest_all_testcases.c.name == basic_testcases.c.name) & (retest_all_testcases.c.testsuite_name == basic_testcases.c.testsuite_name),
        )
        .outerjoin(
            dynamic_testcases,
            (report_parent.c.dynamic_id == dynamic_testcases.c.report_id) & (retest_all_testcases.c.name == dynamic_testcases.c.name) & (retest_all_testcases.c.testsuite_name == dynamic_testcases.c.testsuite_name),
        )
        .outerjoin(
            static_testcases,
            (report_parent.c.static_id == static_testcases.c.report_id) & (retest_all_testcases.c.name == static_testcases.c.name) & (retest_all_testcases.c.testsuite_name == static_testcases.c.testsuite_name),
        )
        .where(retest_all_testcases.c.crashed == False)  # filter suites that are not comparable
        .where((basic_testcases.c.crashed == None) | (basic_testcases.c.crashed == False))
        .where((dynamic_testcases.c.crashed == None) | (dynamic_testcases.c.crashed == False))
        .where((static_testcases.c.crashed == None) | (static_testcases.c.crashed == False))
        .where(retest_all_testcases.c.status != "IGNORED")
    )

    overview_parent = create_materialized_view(
        "TestCaseOverviewParent",
        overview_parent,
        # replace=True,
        metadata=Base.metadata,
    )

    testcase = DBTestCase.__table__

    retest_all_selected = testcase.alias("restest_all_selected")
    basic_selected = testcase.alias("basic_selected")
    dynamic_selected = testcase.alias("dynamic_selected")
    static_selected = testcase.alias("static_selected")

    retest_all_different = testcase.alias("restest_all_different")
    basic_different = testcase.alias("basic_different")
    dynamic_different = testcase.alias("dynamic_different")
    static_different = testcase.alias("static_different")

    testcase_retest_all = testcase.alias("retest_all")
    testcase_basic = testcase.alias("basic")
    testcase_dynamic = testcase.alias("dynamic")
    testcase_static = testcase.alias("static")

    testcases_count = (
        select(
            overview.c.commit,
            count(distinct(retest_all_selected.c.id)).label("retest_all_count"),
            count(distinct(retest_all_different.c.id)).label("retest_all_count_different"),
            count(distinct(basic_selected.c.id)).label("basic_count"),
            count(distinct(basic_different.c.id)).label("basic_count_different"),
            count(distinct(dynamic_selected.c.id)).label("dynamic_count"),
            count(distinct(dynamic_different.c.id)).label("dynamic_count_different"),
            count(distinct(static_selected.c.id)).label("static_count"),
            count(distinct(static_different.c.id)).label("static_count_different"),
        )
        .select_from(overview)
        .outerjoin(
            overview_parent,
            (overview.c.commit == overview_parent.c.commit) & (overview.c.retest_all_suite_name == overview_parent.c.retest_all_suite_name) & (overview.c.retest_all_name == overview_parent.c.retest_all_name),
        )
        .outerjoin(
            retest_all_selected,
            (overview.c.retest_all_testcase_id == retest_all_selected.c.id),
        )
        .outerjoin(
            retest_all_different,
            (overview.c.retest_all_testcase_id == retest_all_different.c.id)
            # & (retest_all_different.c.status == "FAILED"),
            & ~exists(select().select_from(testcase_retest_all).where(overview_parent.c.retest_all_testcase_id == testcase_retest_all.c.id).where(testcase_retest_all.c.status == retest_all_different.c.status).scalar_subquery()),
        )
        .outerjoin(basic_selected, (overview.c.basic_testcase_id == basic_selected.c.id))
        .outerjoin(
            basic_different,
            (overview.c.basic_testcase_id == basic_different.c.id)
            # & (basic_different.c.status == "FAILED"),
            & ~exists(select().select_from(testcase_basic).where(overview_parent.c.basic_testcase_id == testcase_basic.c.id).where(testcase_basic.c.status == basic_different.c.status).scalar_subquery()),
        )
        .outerjoin(dynamic_selected, (overview.c.dynamic_testcase_id == dynamic_selected.c.id))
        .outerjoin(
            dynamic_different,
            (overview.c.dynamic_testcase_id == dynamic_different.c.id)
            # & (dynamic_different.c.status == "FAILED"),
            & ~exists(select().select_from(testcase_dynamic).where(overview_parent.c.dynamic_testcase_id == testcase_dynamic.c.id).where(testcase_dynamic.c.status == dynamic_different.c.status).scalar_subquery()),
        )
        .outerjoin(static_selected, (overview.c.static_testcase_id == static_selected.c.id))
        .outerjoin(
            static_different,
            (overview.c.static_testcase_id == static_different.c.id)
            # & (static_different.c.status == "FAILED"),
            & ~exists(select().select_from(testcase_static).where(overview_parent.c.static_testcase_id == testcase_static.c.id).where(testcase_static.c.status == static_different.c.status).scalar_subquery()),
        )
        .group_by(
            overview.c.commit,
            overview.c.retest_all_id,
            overview.c.basic_id,
            overview.c.dynamic_id,
            overview.c.static_id,
        )
    )

    testcases_count = create_materialized_view(
        "TestCasesCount",
        testcases_count,
        # replace=True,
        metadata=Base.metadata,
    )

    target_count = (
        select(
            overview.c.commit,
            overview.c.target,
            count(distinct(retest_all_selected.c.id)).label("retest_all_count"),
            count(distinct(retest_all_different.c.id)).label("retest_all_count_different"),
            count(distinct(basic_selected.c.id)).label("basic_count"),
            count(distinct(basic_different.c.id)).label("basic_count_different"),
            count(distinct(dynamic_selected.c.id)).label("dynamic_count"),
            count(distinct(dynamic_different.c.id)).label("dynamic_count_different"),
            count(distinct(static_selected.c.id)).label("static_count"),
            count(distinct(static_different.c.id)).label("static_count_different"),
        )
        .select_from(overview)
        .outerjoin(
            overview_parent,
            (overview.c.commit == overview_parent.c.commit) & (overview.c.retest_all_suite_name == overview_parent.c.retest_all_suite_name) & (overview.c.retest_all_name == overview_parent.c.retest_all_name),
        )
        .outerjoin(
            retest_all_selected,
            (overview.c.retest_all_testcase_id == retest_all_selected.c.id),
        )
        .outerjoin(
            retest_all_different,
            (overview.c.retest_all_testcase_id == retest_all_different.c.id)
            # & (retest_all_different.c.status == "FAILED"),
            & ~exists(select().select_from(testcase_retest_all).where(overview_parent.c.retest_all_testcase_id == testcase_retest_all.c.id).where(testcase_retest_all.c.status == retest_all_different.c.status).scalar_subquery()),
        )
        .outerjoin(basic_selected, (overview.c.basic_testcase_id == basic_selected.c.id))
        .outerjoin(
            basic_different,
            (overview.c.basic_testcase_id == basic_different.c.id)
            # & (basic_different.c.status == "FAILED"),
            & ~exists(select().select_from(testcase_basic).where(overview_parent.c.basic_testcase_id == testcase_basic.c.id).where(testcase_basic.c.status == basic_different.c.status).scalar_subquery()),
        )
        .outerjoin(dynamic_selected, (overview.c.dynamic_testcase_id == dynamic_selected.c.id))
        .outerjoin(
            dynamic_different,
            (overview.c.dynamic_testcase_id == dynamic_different.c.id)
            # & (dynamic_different.c.status == "FAILED"),
            & ~exists(select().select_from(testcase_dynamic).where(overview_parent.c.dynamic_testcase_id == testcase_dynamic.c.id).where(testcase_dynamic.c.status == dynamic_different.c.status).scalar_subquery()),
        )
        .outerjoin(static_selected, (overview.c.static_testcase_id == static_selected.c.id))
        .outerjoin(
            static_different,
            (overview.c.static_testcase_id == static_different.c.id)
            # & (static_different.c.status == "FAILED"),
            & ~exists(select().select_from(testcase_static).where(overview_parent.c.static_testcase_id == testcase_static.c.id).where(testcase_static.c.status == static_different.c.status).scalar_subquery()),
        )
        .group_by(
            overview.c.commit,
            overview.c.target,
            overview.c.retest_all_id,
            overview.c.basic_id,
            overview.c.dynamic_id,
            overview.c.static_id,
        )
    )

    target_count = create_materialized_view(
        "TargetCount",
        target_count,
        # replace=True,
        metadata=Base.metadata,
    )

    testcases_selected = (
        select(
            overview.c.commit,
            coalesce(
                aggregate_strings(
                    retest_all_selected.c.name,
                    literal_column("'\n'"),
                ),
                "",
            ).label("retest_all"),
            coalesce(
                aggregate_strings(
                    basic_selected.c.name,
                    literal_column("'\n'"),
                ),
                "",
            ).label("basic"),
            coalesce(
                aggregate_strings(
                    dynamic_selected.c.name,
                    literal_column("'\n'"),
                ),
                "",
            ).label("dynamic"),
            coalesce(
                aggregate_strings(
                    static_selected.c.name,
                    literal_column("'\n'"),
                ),
                "",
            ).label("static"),
        )
        .select_from(overview)
        .outerjoin(
            retest_all_selected,
            overview.c.retest_all_testcase_id == retest_all_selected.c.id,
        )
        .outerjoin(
            basic_selected,
            overview.c.basic_testcase_id == basic_selected.c.id,
        )
        .outerjoin(
            dynamic_selected,
            overview.c.dynamic_testcase_id == dynamic_selected.c.id,
        )
        .outerjoin(
            static_selected,
            overview.c.static_testcase_id == static_selected.c.id,
        )
        .group_by(
            overview.c.commit,
            overview.c.retest_all_id,
            overview.c.basic_id,
            overview.c.dynamic_id,
            overview.c.static_id,
        )
    )

    testcases_selected = create_materialized_view(
        "TestCasesSelected",
        testcases_selected,
        # replace=True,
        metadata=Base.metadata,
    )

    testcases_different = (
        select(
            overview.c.commit,
            coalesce(
                aggregate_strings(
                    retest_all_selected.c.name,
                    literal_column("'\n'"),
                ),
                "",
            ).label("retest_all"),
            coalesce(
                aggregate_strings(
                    basic_selected.c.name,
                    literal_column("'\n'"),
                ),
                "",
            ).label("basic"),
            coalesce(
                aggregate_strings(
                    dynamic_selected.c.name,
                    literal_column("'\n'"),
                ),
                "",
            ).label("dynamic"),
            coalesce(
                aggregate_strings(
                    static_selected.c.name,
                    literal_column("'\n'"),
                ),
                "",
            ).label("static"),
        )
        .select_from(overview)
        .outerjoin(
            overview_parent,
            (overview.c.commit == overview_parent.c.commit) & (overview.c.retest_all_suite_name == overview_parent.c.retest_all_suite_name) & (overview.c.retest_all_name == overview_parent.c.retest_all_name),
        )
        .outerjoin(
            retest_all_selected,
            (overview.c.retest_all_testcase_id == retest_all_selected.c.id)
            # & (retest_all_selected.c.status == "FAILED"),
            & ~exists(select().select_from(testcase_retest_all).where(overview_parent.c.retest_all_testcase_id == testcase_retest_all.c.id).where(testcase_retest_all.c.status == retest_all_selected.c.status).scalar_subquery()),
        )
        .outerjoin(
            basic_selected,
            (overview.c.basic_testcase_id == basic_selected.c.id)
            # & (basic_selected.c.status == "FAILED"),
            & ~exists(select().select_from(testcase_basic).where(overview_parent.c.basic_testcase_id == testcase_basic.c.id).where(testcase_basic.c.status == basic_selected.c.status).scalar_subquery()),
        )
        .outerjoin(
            dynamic_selected,
            (overview.c.dynamic_testcase_id == dynamic_selected.c.id)
            # & (dynamic_selected.c.status == "FAILED"),
            & ~exists(select().select_from(testcase_dynamic).where(overview_parent.c.dynamic_testcase_id == testcase_dynamic.c.id).where(testcase_dynamic.c.status == dynamic_selected.c.status).scalar_subquery()),
        )
        .outerjoin(
            static_selected,
            (overview.c.static_testcase_id == static_selected.c.id)
            # & (static_selected.c.status == "FAILED"),
            & ~exists(select().select_from(testcase_static).where(overview_parent.c.static_testcase_id == testcase_static.c.id).where(testcase_static.c.status == static_selected.c.status).scalar_subquery()),
        )
        .group_by(
            overview.c.commit,
            overview.c.retest_all_id,
            overview.c.dynamic_id,
            overview.c.static_id,
        )
    )

    testcases_different = create_materialized_view(
        "TestCasesDifferent",
        testcases_different,
        # replace=True,
        metadata=Base.metadata,
    )

    duration_1 = (
        select(
            repository.c.path,
            repository.c.id.label("repo_id"),
            func.round(func.cast(func.avg(report.c.retest_all_test_duration), NUMERIC), 2).label("retest_all_mean"),
            func.round(func.cast(func.stddev(report.c.retest_all_test_duration), NUMERIC), 2).label("retest_all_stddev"),
            func.round(func.cast(func.avg(report.c.basic_test_duration), NUMERIC), 2).label("basic_mean"),
            func.round(func.cast(func.stddev(report.c.basic_test_duration), NUMERIC), 2).label("basic_stddev"),
            func.round(func.cast(func.avg(report.c.dynamic_test_duration), NUMERIC), 2).label("dynamic_mean"),
            func.round(func.cast(func.stddev(report.c.dynamic_test_duration), NUMERIC), 2).label("dynamic_stddev"),
            func.round(func.cast(func.avg(report.c.static_test_duration), NUMERIC), 2).label("static_mean"),
            func.round(func.cast(func.stddev(report.c.static_test_duration), NUMERIC), 2).label("static_stddev"),
            func.round(
                func.cast(
                    func.avg(report.c.basic_test_duration * 100.0 / report.c.retest_all_test_duration),
                    NUMERIC,
                ),
                2,
            ).label("basic_mean_relative"),
            func.round(
                func.cast(
                    func.stddev(report.c.basic_test_duration * 100.0 / report.c.retest_all_test_duration),
                    NUMERIC,
                ),
                2,
            ).label("basic_stddev_relative"),
            func.round(
                func.cast(
                    func.avg(report.c.dynamic_test_duration * 100.0 / report.c.retest_all_test_duration),
                    NUMERIC,
                ),
                2,
            ).label("dynamic_mean_relative"),
            func.round(
                func.cast(
                    func.stddev(report.c.dynamic_test_duration * 100.0 / report.c.retest_all_test_duration),
                    NUMERIC,
                ),
                2,
            ).label("dynamic_stddev_relative"),
            func.round(
                func.cast(
                    func.avg(report.c.static_test_duration * 100.0 / report.c.retest_all_test_duration),
                    NUMERIC,
                ),
                2,
            ).label("static_mean_relative"),
            func.round(
                func.cast(
                    func.stddev(report.c.static_test_duration * 100.0 / report.c.retest_all_test_duration),
                    NUMERIC,
                ),
                2,
            ).label("static_stddev_relative"),
        )
        .select_from(report, commit, repository)
        .where(report.c.commit == commit.c.id)
        .where(repository.c.id == commit.c.repo_id)
        .group_by(repository.c.id)
    )

    duration_2 = select(
        literal_column("'all'"),
        literal_column("NULL").label("repo_id"),
        func.round(func.cast(func.avg(report.c.retest_all_test_duration), NUMERIC), 2).label("retest_all_mean"),
        func.round(func.cast(func.stddev(report.c.retest_all_test_duration), NUMERIC), 2).label("retest_all_stddev"),
        func.round(func.cast(func.avg(report.c.basic_test_duration), NUMERIC), 2).label("basic_mean"),
        func.round(func.cast(func.stddev(report.c.basic_test_duration), NUMERIC), 2).label("basic_stddev"),
        func.round(func.cast(func.avg(report.c.dynamic_test_duration), NUMERIC), 2).label("dynamic_mean"),
        func.round(func.cast(func.stddev(report.c.dynamic_test_duration), NUMERIC), 2).label("dynamic_stddev"),
        func.round(func.cast(func.avg(report.c.static_test_duration), NUMERIC), 2).label("static_mean"),
        func.round(func.cast(func.stddev(report.c.static_test_duration), NUMERIC), 2).label("static_stddev"),
        func.round(
            func.cast(
                func.avg(report.c.basic_test_duration * 100.0 / report.c.retest_all_test_duration),
                NUMERIC,
            ),
            2,
        ).label("basic_mean_relative"),
        func.round(
            func.cast(
                func.stddev(report.c.basic_test_duration * 100.0 / report.c.retest_all_test_duration),
                NUMERIC,
            ),
            2,
        ).label("basic_stddev_relative"),
        func.round(
            func.cast(
                func.avg(report.c.dynamic_test_duration * 100.0 / report.c.retest_all_test_duration),
                NUMERIC,
            ),
            2,
        ).label("dynamic_mean_relative"),
        func.round(
            func.cast(
                func.stddev(report.c.dynamic_test_duration * 100.0 / report.c.retest_all_test_duration),
                NUMERIC,
            ),
            2,
        ).label("dynamic_stddev_relative"),
        func.round(
            func.cast(
                func.avg(report.c.static_test_duration * 100.0 / report.c.retest_all_test_duration),
                NUMERIC,
            ),
            2,
        ).label("static_mean_relative"),
        func.round(
            func.cast(
                func.stddev(report.c.static_test_duration * 100.0 / report.c.retest_all_test_duration),
                NUMERIC,
            ),
            2,
        ).label("static_stddev_relative"),
    ).select_from(report)

    duration = create_materialized_view(
        "Duration",
        duration_1.union(duration_2),
        # replace=True,
        metadata=Base.metadata,
    )

    total_repos = select(
        literal_column("CAST('1' as int)").label("id"),
        literal_column("'HistoryTotalRepos'").label("macro"),
        func.count(distinct(report.c.repo_id).label("value")),
    ).select_from(report)
    number_commits_total = select(
        literal_column("'2'"),
        literal_column("'HistoryNumberCommitsTotal'"),
        func.count(report.c.commit),
    ).select_from(report)
    number_commits_per_repo_sub = select(func.count(report.c.commit).label("count")).select_from(report).group_by(report.c.repo_id).subquery()
    number_commits_per_repo = select(
        literal_column("'3'"),
        literal_column("'HistoryNumberCommitsPerRepo'"),
        func.avg(number_commits_per_repo_sub.c.count),
    ).select_from(number_commits_per_repo_sub)

    total_retest_all = select(
        literal_column("'4'"),
        literal_column("'HistoryTotalRetestAll'"),
        func.sum(testcases_count.c.retest_all_count),
    ).select_from(testcases_count)
    total_basic = select(
        literal_column("'5'"),
        literal_column("'HistoryTotalBasic'"),
        func.sum(testcases_count.c.basic_count),
    ).select_from(testcases_count)
    total_dynamic = select(
        literal_column("'6'"),
        literal_column("'HistoryTotalDynamic'"),
        func.sum(testcases_count.c.dynamic_count),
    ).select_from(testcases_count)
    total_static = select(
        literal_column("'7'"),
        literal_column("'HistoryTotalStatic'"),
        func.sum(testcases_count.c.static_count),
    ).select_from(testcases_count)

    retest_all_different = select(
        literal_column("'8'"),
        literal_column("'HistoryDifferentRetestAll'"),
        func.sum(testcases_count.c.retest_all_count_different),
    ).select_from(testcases_count)
    basic_different = select(
        literal_column("'9'"),
        literal_column("'HistoryDifferentBasic'"),
        func.sum(testcases_count.c.basic_count_different),
    ).select_from(testcases_count)
    dynamic_different = select(
        literal_column("'10'"),
        literal_column("'HistoryDifferentDynamic'"),
        func.sum(testcases_count.c.dynamic_count_different),
    ).select_from(testcases_count)
    static_different = select(
        literal_column("'11'"),
        literal_column("'HistoryDifferentStatic'"),
        func.sum(testcases_count.c.static_count_different),
    ).select_from(testcases_count)

    relative_basic = select(
        literal_column("'12'"),
        literal_column("'HistoryRelativeBasic'"),
        func.round(
            func.cast(
                func.sum(testcases_count.c.basic_count) / func.sum(testcases_count.c.retest_all_count) * 100.0,
                NUMERIC,
            ),
            2,
        ),
    ).select_from(testcases_count)
    relative_dynamic = select(
        literal_column("'13'"),
        literal_column("'HistoryRelativeDynamic'"),
        func.round(
            func.cast(
                func.sum(testcases_count.c.dynamic_count) / func.sum(testcases_count.c.retest_all_count) * 100.0,
                NUMERIC,
            ),
            2,
        ),
    ).select_from(testcases_count)
    relative_static = select(
        literal_column("'14'"),
        literal_column("'HistoryRelativeStatic'"),
        func.round(
            func.cast(
                func.sum(testcases_count.c.static_count) / func.sum(testcases_count.c.retest_all_count) * 100.0,
                NUMERIC,
            ),
            2,
        ),
    ).select_from(testcases_count)

    unit_retest_all = (
        select(
            literal_column("'15'"),
            literal_column("'HistoryUnitRetestAll'"),
            func.sum(target_count.c.retest_all_count),
        )
        .select_from(target_count)
        .where(target_count.c.target == "UNIT")
    )
    unit_basic = (
        select(
            literal_column("'16'"),
            literal_column("'HistoryUnitBasic'"),
            func.sum(target_count.c.basic_count),
        )
        .select_from(target_count)
        .where(target_count.c.target == "UNIT")
    )
    unit_dynamic = (
        select(
            literal_column("'17'"),
            literal_column("'HistoryUnitDynamic'"),
            func.sum(target_count.c.dynamic_count),
        )
        .select_from(target_count)
        .where(target_count.c.target == "UNIT")
    )
    unit_static = (
        select(
            literal_column("'18'"),
            literal_column("'HistoryUnitStatic'"),
            func.sum(target_count.c.static_count),
        )
        .select_from(target_count)
        .where(target_count.c.target == "UNIT")
    )

    unit_relative_basic = (
        select(
            literal_column("'19'"),
            literal_column("'HistoryUnitRelativeBasic'"),
            func.round(
                func.cast(
                    func.sum(target_count.c.basic_count) / func.sum(target_count.c.retest_all_count) * 100.0,
                    NUMERIC,
                ),
                2,
            ),
        )
        .select_from(target_count)
        .where(target_count.c.target == "UNIT")
    )
    unit_relative_dynamic = (
        select(
            literal_column("'20'"),
            literal_column("'HistoryUnitRelativeDynamic'"),
            func.round(
                func.cast(
                    func.sum(target_count.c.dynamic_count) / func.sum(target_count.c.retest_all_count) * 100.0,
                    NUMERIC,
                ),
                2,
            ),
        )
        .select_from(target_count)
        .where(target_count.c.target == "UNIT")
    )
    unit_relative_static = (
        select(
            literal_column("'21'"),
            literal_column("'HistoryUnitRelativeStatic'"),
            func.round(
                func.cast(
                    func.sum(target_count.c.static_count) / func.sum(target_count.c.retest_all_count) * 100.0,
                    NUMERIC,
                ),
                2,
            ),
        )
        .select_from(target_count)
        .where(target_count.c.target == "UNIT")
    )

    integration_retest_all = (
        select(
            literal_column("'22'"),
            literal_column("'HistoryIntegrationRetestAll'"),
            func.sum(target_count.c.retest_all_count),
        )
        .select_from(target_count)
        .where(target_count.c.target == "INTEGRATION")
    )
    integration_basic = (
        select(
            literal_column("'23'"),
            literal_column("'HistoryIntegrationBasic'"),
            func.sum(target_count.c.basic_count),
        )
        .select_from(target_count)
        .where(target_count.c.target == "INTEGRATION")
    )
    integration_dynamic = (
        select(
            literal_column("'24'"),
            literal_column("'HistoryIntegrationDynamic'"),
            func.sum(target_count.c.dynamic_count),
        )
        .select_from(target_count)
        .where(target_count.c.target == "INTEGRATION")
    )
    integration_static = (
        select(
            literal_column("'25'"),
            literal_column("'HistoryIntegrationStatic'"),
            func.sum(target_count.c.static_count),
        )
        .select_from(target_count)
        .where(target_count.c.target == "INTEGRATION")
    )

    integration_relative_basic = (
        select(
            literal_column("'26'"),
            literal_column("'HistoryIntegrationRelativeBasic'"),
            func.round(
                func.cast(
                    func.sum(target_count.c.basic_count) / func.sum(target_count.c.retest_all_count) * 100.0,
                    NUMERIC,
                ),
                2,
            ),
        )
        .select_from(target_count)
        .where(target_count.c.target == "INTEGRATION")
    )
    integration_relative_dynamic = (
        select(
            literal_column("'27'"),
            literal_column("'HistoryIntegrationRelativeDynamic'"),
            func.round(
                func.cast(
                    func.sum(target_count.c.dynamic_count) / func.sum(target_count.c.retest_all_count) * 100.0,
                    NUMERIC,
                ),
                2,
            ),
        )
        .select_from(target_count)
        .where(target_count.c.target == "INTEGRATION")
    )
    integration_relative_static = (
        select(
            literal_column("'28'"),
            literal_column("'HistoryIntegrationRelativeStatic'"),
            func.round(
                func.cast(
                    func.sum(target_count.c.static_count) / func.sum(target_count.c.retest_all_count) * 100.0,
                    NUMERIC,
                ),
                2,
            ),
        )
        .select_from(target_count)
        .where(target_count.c.target == "INTEGRATION")
    )

    doc_retest_all = (
        select(
            literal_column("'29'"),
            literal_column("'HistoryDoctestRetestAll'"),
            func.sum(target_count.c.retest_all_count),
        )
        .select_from(target_count)
        .where(target_count.c.target == "DOCTEST")
    )
    doc_basic = (
        select(
            literal_column("'30'"),
            literal_column("'HistoryDoctestBasic'"),
            func.sum(target_count.c.basic_count),
        )
        .select_from(target_count)
        .where(target_count.c.target == "DOCTEST")
    )
    doc_dynamic = (
        select(
            literal_column("'31'"),
            literal_column("'HistoryDoctestDynamic'"),
            func.sum(target_count.c.dynamic_count),
        )
        .select_from(target_count)
        .where(target_count.c.target == "DOCTEST")
    )
    doc_static = (
        select(
            literal_column("'32'"),
            literal_column("'HistoryDoctestStatic'"),
            func.sum(target_count.c.static_count),
        )
        .select_from(target_count)
        .where(target_count.c.target == "DOCTEST")
    )

    doc_relative_basic = (
        select(
            literal_column("'33'"),
            literal_column("'HistoryDoctestRelativeBasic'"),
            func.round(
                func.cast(
                    func.sum(target_count.c.basic_count) / func.sum(target_count.c.retest_all_count) * 100.0,
                    NUMERIC,
                ),
                2,
            ),
        )
        .select_from(target_count)
        .where(target_count.c.target == "DOCTEST")
    )
    doc_relative_dynamic = (
        select(
            literal_column("'34'"),
            literal_column("'HistoryDoctestRelativeDynamic'"),
            func.round(
                func.cast(
                    func.sum(target_count.c.dynamic_count) / func.sum(target_count.c.retest_all_count) * 100.0,
                    NUMERIC,
                ),
                2,
            ),
        )
        .select_from(target_count)
        .where(target_count.c.target == "DOCTEST")
    )
    doc_relative_static = (
        select(
            literal_column("'35'"),
            literal_column("'HistoryDoctestRelativeStatic'"),
            func.round(
                func.cast(
                    func.sum(target_count.c.static_count) / func.sum(target_count.c.retest_all_count) * 100.0,
                    NUMERIC,
                ),
                2,
            ),
        )
        .select_from(target_count)
        .where(target_count.c.target == "DOCTEST")
    )

    average_testing_time = (
        select(
            literal_column("36"),
            literal_column("'HistoryAverageTestingTime'"),
            duration.c.retest_all_mean,
        )
        .select_from(duration)
        .where(duration.c.path == "all")
    )
    average_testing_time_min = (
        select(
            literal_column("37"),
            literal_column("'HistoryAverageTestingTimeMin'"),
            func.min(duration.c.retest_all_mean),
        )
        .select_from(duration)
        .where(duration.c.path != "all")
    )
    average_testing_time_max = (
        select(
            literal_column("38"),
            literal_column("'HistoryAverageTestingTimeMax'"),
            func.max(duration.c.retest_all_mean),
        )
        .select_from(duration)
        .where(duration.c.path != "all")
    )

    efficiency_basic = (
        select(
            literal_column("39"),
            literal_column("'HistoryEfficiencyBasic'"),
            duration.c.basic_mean_relative,
        )
        .select_from(duration)
        .where(duration.c.path == "all")
    )
    efficiency_basic_min = (
        select(
            literal_column("40"),
            literal_column("'HistoryEfficiencyBasicMin'"),
            func.min(duration.c.basic_mean_relative),
        )
        .select_from(duration)
        .where(duration.c.path != "all")
    )
    efficiency_basic_max = (
        select(
            literal_column("41"),
            literal_column("'HistoryEfficiencyBasicMax'"),
            func.max(duration.c.basic_mean_relative),
        )
        .select_from(duration)
        .where(duration.c.path != "all")
    )

    efficiency_dynamic = (
        select(
            literal_column("42"),
            literal_column("'HistoryEfficiencyDynamic'"),
            duration.c.dynamic_mean_relative,
        )
        .select_from(duration)
        .where(duration.c.path == "all")
    )
    efficiency_dynamic_min = (
        select(
            literal_column("43"),
            literal_column("'HistoryEfficiencyDynamicMin'"),
            func.min(duration.c.dynamic_mean_relative),
        )
        .select_from(duration)
        .where(duration.c.path != "all")
    )
    efficiency_dynamic_max = (
        select(
            literal_column("44"),
            literal_column("'HistoryEfficiencyDynamicMax'"),
            func.max(duration.c.dynamic_mean_relative),
        )
        .select_from(duration)
        .where(duration.c.path != "all")
    )

    efficiency_static = (
        select(
            literal_column("45"),
            literal_column("'HistoryEfficiencyStatic'"),
            duration.c.static_mean_relative,
        )
        .select_from(duration)
        .where(duration.c.path == "all")
    )
    efficiency_static_min = (
        select(
            literal_column("46"),
            literal_column("'HistoryEfficiencyStaticMin'"),
            func.min(duration.c.static_mean_relative),
        )
        .select_from(duration)
        .where(duration.c.path != "all")
    )
    efficiency_static_max = (
        select(
            literal_column("47"),
            literal_column("'HistoryEfficiencyStaticMax'"),
            func.max(duration.c.static_mean_relative),
        )
        .select_from(duration)
        .where(duration.c.path != "all")
    )

    efficiency_better_basic = (
        select(
            literal_column("48"),
            literal_column("'HistoryEfficiencyBetterBasic'"),
            0 + func.sum(1),
        )
        .select_from(duration)
        .where(duration.c.path != "all")
        .where(duration.c.basic_mean_relative < 100.0)
    )
    efficiency_better_dynamic = (
        select(
            literal_column("49"),
            literal_column("'HistoryEfficiencyBetterDynamic'"),
            0 + func.sum(1),
        )
        .select_from(duration)
        .where(duration.c.path != "all")
        .where(duration.c.dynamic_mean_relative < 100.0)
    )
    efficiency_better_static = (
        select(
            literal_column("50"),
            literal_column("'HistoryEfficiencyBetterStatic'"),
            0 + func.sum(1),
        )
        .select_from(duration)
        .where(duration.c.path != "all")
        .where(duration.c.static_mean_relative < 100.0)
    )

    build_overhead_basic = select(
        literal_column("51"),
        literal_column("'HistoryBuildOverheadBasic'"),
        func.round(
            func.cast(
                func.avg(report.c.basic_build_duration / report.c.retest_all_build_duration) * 100.0 - 100.0,
                NUMERIC,
            ),
            2,
        ),
    ).select_from(report_parent)

    build_overhead_dynamic = select(
        literal_column("52"),
        literal_column("'HistoryBuildOverheadDynamic'"),
        func.round(
            func.cast(
                func.avg(report.c.dynamic_build_duration / report.c.retest_all_build_duration) * 100.0 - 100.0,
                NUMERIC,
            ),
            2,
        ),
    ).select_from(report_parent)
    build_overhead_static = select(
        literal_column("53"),
        literal_column("'HistoryBuildOverheadStatic'"),
        func.round(
            func.cast(
                func.avg(report.c.static_build_duration / report.c.retest_all_build_duration) * 100.0 - 100.0,
                NUMERIC,
            ),
            2,
        ),
    ).select_from(report_parent)

    build_overhead_basic_parent = select(
        literal_column("54"),
        literal_column("'HistoryBuildOverheadParentBasic'"),
        func.round(
            func.cast(
                func.avg(report_parent.c.basic_build_duration / report_parent.c.retest_all_build_duration) * 100.0 - 100.0,
                NUMERIC,
            ),
            2,
        ),
    ).select_from(report_parent)
    build_overhead_dynamic_parent = select(
        literal_column("55"),
        literal_column("'HistoryBuildOverheadParentDynamic'"),
        func.round(
            func.cast(
                func.avg(report_parent.c.dynamic_build_duration / report_parent.c.retest_all_build_duration) * 100.0 - 100.0,
                NUMERIC,
            ),
            2,
        ),
    ).select_from(report_parent)
    build_overhead_static_parent = select(
        literal_column("56"),
        literal_column("'HistoryBuildOverheadParentStatic'"),
        func.round(
            func.cast(
                func.avg(report_parent.c.static_build_duration / report_parent.c.retest_all_build_duration) * 100.0 - 100.0,
                NUMERIC,
            ),
            2,
        ),
    ).select_from(report_parent)

    test_overhead_basic = select(
        literal_column("57"),
        literal_column("'HistoryTestOverheadBasic'"),
        func.round(
            func.cast(
                func.avg(report.c.basic_test_duration / report.c.retest_all_test_duration) * 100.0 - 100.0,
                NUMERIC,
            ),
            2,
        ),
    ).select_from(report_parent)
    test_overhead_dynamic = select(
        literal_column("58"),
        literal_column("'HistoryTestOverheadDynamic'"),
        func.round(
            func.cast(
                func.avg(report.c.dynamic_test_duration / report.c.retest_all_test_duration) * 100.0 - 100.0,
                NUMERIC,
            ),
            2,
        ),
    ).select_from(report_parent)
    test_overhead_static = select(
        literal_column("59"),
        literal_column("'HistoryTestOverheadStatic'"),
        func.round(
            func.cast(
                func.avg(report.c.static_test_duration / report.c.retest_all_test_duration) * 100.0 - 100.0,
                NUMERIC,
            ),
            2,
        ),
    ).select_from(report_parent)

    test_overhead_basic_parent = select(
        literal_column("60"),
        literal_column("'HistoryTestOverheadParentBasic'"),
        func.round(
            func.cast(
                func.avg(report_parent.c.basic_test_duration / report_parent.c.retest_all_test_duration) * 100.0 - 100.0,
                NUMERIC,
            ),
            2,
        ),
    ).select_from(report_parent)
    test_overhead_dynamic_parent = select(
        literal_column("61"),
        literal_column("'HistoryTestOverheadParentDynamic'"),
        func.round(
            func.cast(
                func.avg(report_parent.c.dynamic_test_duration / report_parent.c.retest_all_test_duration) * 100.0 - 100.0,
                NUMERIC,
            ),
            2,
        ),
    ).select_from(report_parent)
    test_overhead_static_parent = select(
        literal_column("62"),
        literal_column("'HistoryTestOverheadParentStatic'"),
        func.round(
            func.cast(
                func.avg(report_parent.c.static_test_duration / report_parent.c.retest_all_test_duration) * 100.0 - 100.0,
                NUMERIC,
            ),
            2,
        ),
    ).select_from(report_parent)

    facts = total_repos.union(
        number_commits_total,
        number_commits_per_repo,
        total_retest_all,
        total_basic,
        total_dynamic,
        total_static,
        retest_all_different,
        basic_different,
        dynamic_different,
        static_different,
        relative_basic,
        relative_dynamic,
        relative_static,
        unit_retest_all,
        unit_basic,
        unit_dynamic,
        unit_static,
        unit_relative_basic,
        unit_relative_dynamic,
        unit_relative_static,
        integration_retest_all,
        integration_basic,
        integration_dynamic,
        integration_static,
        integration_relative_basic,
        integration_relative_dynamic,
        integration_relative_static,
        doc_retest_all,
        doc_basic,
        doc_dynamic,
        doc_static,
        doc_relative_basic,
        doc_relative_dynamic,
        doc_relative_static,
        average_testing_time,
        average_testing_time_min,
        average_testing_time_max,
        efficiency_basic,
        efficiency_dynamic,
        efficiency_basic_min,
        efficiency_dynamic_min,
        efficiency_basic_max,
        efficiency_dynamic_max,
        efficiency_static,
        efficiency_static_min,
        efficiency_static_max,
        efficiency_better_basic,
        efficiency_better_dynamic,
        efficiency_better_static,
        build_overhead_basic,
        build_overhead_dynamic,
        build_overhead_static,
        build_overhead_basic_parent,
        build_overhead_dynamic_parent,
        build_overhead_static_parent,
        test_overhead_basic,
        test_overhead_dynamic,
        test_overhead_static,
        test_overhead_basic_parent,
        test_overhead_dynamic_parent,
        test_overhead_static_parent,
    )

    facts = create_materialized_view(
        "Facts",
        facts,
        # replace=True,
        metadata=Base.metadata,
    )

    return HistoryViewInformation(
        overview,
        duration,
        report,
        target_count,
        testcases_count,
        testcases_different,
        testcases_selected,
        statistics_commit,
        statistics_repository,
    )
