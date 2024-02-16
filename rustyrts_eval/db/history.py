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


class DBTestReportMeta(Base.__class__, TestReport.__class__):
    ...


class DBTestSuiteMeta(Base.__class__, TestSuite.__class__):
    ...


class DBTestCaseMeta(Base.__class__, TestCase.__class__):
    ...


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
    suites: Mapped[List["DBTestSuite"]] = relationship(
        "DBTestSuite", back_populates="report"
    )
    commit_str = Column(String, nullable=False)
    commit_id = Column(
        Integer, ForeignKey("{}.id".format(DBCommit.__tablename__), ondelete="CASCADE")
    )
    commit: Mapped[DBCommit] = relationship("DBCommit", back_populates="reports")
    log = Column(Text)
    has_failed = Column(Boolean)
    has_errored = Column(Boolean)

    __table_args__ = tuple(
        [UniqueConstraint("name", "commit_str", name="_test_name_revision_uc")]
    )

    @classmethod
    def get_single(
        cls, name: str, commit_str: str, session: Session
    ) -> Optional["DBTestReport"]:
        db_report: Optional[DBTestReport] = (
            session.query(DBTestReport)
            .filter_by(name=name, commit_str=commit_str)
            .first()
        )
        return db_report

    @classmethod
    def create_or_update(cls, report: TestReport, session: Session) -> "DBTestReport":
        # get report from DB
        db_report: Optional[DBTestReport] = cls.get_single(
            name=report.name, commit_str=report.commit_str, session=session
        )

        # create DB report object if not in DB yet
        if not db_report:
            # get commits if exists, otherwise create
            if report.commit:
                report.commit = DBCommit.create_or_get(report.commit, session)

            db_report = DBTestReport.from_domain(report)
            session.add(db_report)
        else:
            # if already existing, update all fields
            db_report.duration = (
                report.duration if report.duration else db_report.duration
            )
            db_report.build_duration = (
                report.build_duration
                if report.build_duration
                else db_report.build_duration
            )
            db_report.commit_str = (
                report.commit_str if report.commit_str else db_report.commit_str
            )
            # get from db if it exists
            db_report.commit = DBCommit.create_or_get(report.commit, session)
            db_report.suites = (
                [DBTestSuite.from_domain(s) for s in report.suites]
                if report.suites is not None
                else db_report.suites
            )
            db_report.log = report.log if report.log else db_report.log
            db_report.has_failed = (
                report.has_failed
                if report.has_failed is not None
                else db_report.has_failed
            )
            db_report.has_errored = (
                report.has_errored
                if report.has_errored is not None
                else db_report.has_errored
            )
        return db_report

    @classmethod
    def from_domain(cls, report: TestReport) -> "DBTestReport":
        if isinstance(report, cls) or not report:
            return report
        return cls(
            name=report.name,
            duration=report.duration,
            build_duration=report.build_duration,
            suites=[]
            if report.suites is None
            else [DBTestSuite.from_domain(suite) for suite in report.suites],
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
        ForeignKey("{}.id".format(DBTestReport.__tablename__), ondelete="CASCADE"),
    )
    report = relationship("DBTestReport", back_populates="suites")
    cases: Mapped[List["DBTestCase"]] = relationship(
        "DBTestCase", back_populates="suite", cascade="all, delete-orphan"
    )

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
        ForeignKey("{}.id".format(DBTestSuite.__tablename__), ondelete="CASCADE"),
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


def register_views_individual(special):
    commit = DBCommit.__table__
    report = DBTestReport.__table__

    retest_all = report.alias("retest_all")
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
            dynamic,
            static,
        )
        .where(commit.c.id == retest_all.c.commit_id)
        .where(commit.c.id == dynamic.c.commit_id)
        .where(commit.c.id == static.c.commit_id)
        .where(retest_all.c.name == f"cargo test{special}")
        .where(dynamic.c.name == f"cargo rustyrts dynamic{special}")
        .where(static.c.name == f"cargo rustyrts static{special}")
        .where(retest_all.c.has_errored == False)
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
            dynamic,
            static,
        )
        .where(commit.c.id == retest_all.c.commit_id)
        .where(commit.c.id == dynamic.c.commit_id)
        .where(commit.c.id == static.c.commit_id)
        .where(retest_all.c.name == f"cargo test{special} - parent")
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

    check_parsed_tests = (
        select(
            suite,
            count(case.c.id).label("count_cases"),
            (
                suite.c.passed_count
                + suite.c.failed_count
                + suite.c.measured_count
                - count(case.c.id)
            ).label("discrepancy"),
        )
        .select_from(suite, case)
        .where(suite.c.id == case.c.suite_id)
        .where(case.c.status != "IGNORED")
        .group_by(suite)
        .having(
            count(case.c.id)
            != (suite.c.passed_count + suite.c.failed_count + suite.c.measured_count)
        )
    )

    check_parsed_tests = create_view(
        "CheckParsedTests", check_parsed_tests, replace=True, metadata=Base.metadata
    )

    statistics_commit = (
        select(
            commit.c.id,
            commit.c.repo_id,
            commit.c.commit_str,
            commit.c.nr_lines.label("lines"),
            commit.c.nr_files.label("files"),
            count(distinct(suite.c.id)).label("suites"),
            sum(
                select(count(distinct(case.c.id)))
                .select_from(case)
                .where(case.c.suite_id == suite.c.id)
                .where(case.c.status != "IGNORED")
                .scalar_subquery()
            ).label("cases"),
            sum(
                select(count(distinct(case.c.id)))
                .select_from(case)
                .where(case.c.suite_id == suite.c.id)
                .where(case.c.target == "UNIT")
                .where(case.c.status != "IGNORED")
                .scalar_subquery()
            ).label("unit"),
            sum(
                select(count(distinct(case.c.id)))
                .select_from(case)
                .where(case.c.suite_id == suite.c.id)
                .where(case.c.target == "INTEGRATION")
                .where(case.c.status != "IGNORED")
                .scalar_subquery()
            ).label("integration"),
            func.round(func.cast(sum(suite.c.duration), NUMERIC), 2).label("duration"),
        )
        .select_from(commit, report, suite)
        .where(commit.c.id == report.c.commit_id)
        .where(report.c.id == suite.c.report_id)
        .where(report.c.has_errored == False)
        .where(report.c.name == "cargo test")
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
            func.cast(func.avg(statistics_commit.c.lines), NUMERIC).label("avg_lines"),
            func.cast(func.avg(statistics_commit.c.files), NUMERIC).label("avg_files"),
            func.cast(func.avg(statistics_commit.c.suites), NUMERIC).label(
                "avg_suites"
            ),
            func.cast(func.avg(statistics_commit.c.cases), NUMERIC).label("avg_cases"),
            func.cast(func.avg(statistics_commit.c.unit), NUMERIC).label("avg_unit"),
            func.cast(func.avg(statistics_commit.c.integration), NUMERIC).label(
                "avg_integration"
            ),
            func.cast(func.avg(statistics_commit.c.duration), NUMERIC).label(
                "avg_duration"
            ),
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

    special = " sequential" if sequential else ""
    report, report_parent = register_views_individual(special)

    report = create_view(
        "TestReportExtended",
        report,
        replace=True,
        metadata=Base.metadata,
    )

    retest_all = report.alias("retest_all")
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
    dynamic_testcases = testcase.alias("dynamic_test_cases")
    static_testcases = testcase.alias("static_test_cases")

    overview = (
        select(
            report.c.commit,
            report.c.retest_all_id.label("retest_all_id"),
            report.c.dynamic_id.label("dynamic_id"),
            report.c.static_id.label("static_id"),
            retest_all_testcases.c.target.label("target"),
            retest_all_testcases.c.testsuite_name.label("retest_all_suite_name"),
            retest_all_testcases.c.name.label("retest_all_name"),
            retest_all_testcases.c.id.label("retest_all_testcase_id"),
            retest_all_testcases.c.status.label("retest_all_status"),
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
            dynamic_testcases,
            (report.c.dynamic_id == dynamic_testcases.c.report_id)
            & (retest_all_testcases.c.name == dynamic_testcases.c.name)
            & (
                retest_all_testcases.c.testsuite_name
                == dynamic_testcases.c.testsuite_name
            ),
        )
        .outerjoin(
            static_testcases,
            (report.c.static_id == static_testcases.c.report_id)
            & (retest_all_testcases.c.name == static_testcases.c.name)
            & (
                retest_all_testcases.c.testsuite_name
                == static_testcases.c.testsuite_name
            ),
        )
        .where(
            retest_all_testcases.c.crashed == False
        )  # filter suites that are not comparable
        .where(
            (dynamic_testcases.c.crashed == None)
            | (dynamic_testcases.c.crashed == False)
        )
        .where(
            (static_testcases.c.crashed == None) | (static_testcases.c.crashed == False)
        )
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
            report_parent.c.dynamic_id.label("dynamic_id"),
            report_parent.c.static_id.label("static_id"),
            retest_all_testcases.c.target.label("target"),
            retest_all_testcases.c.testsuite_name.label("retest_all_suite_name"),
            retest_all_testcases.c.name.label("retest_all_name"),
            retest_all_testcases.c.id.label("retest_all_testcase_id"),
            retest_all_testcases.c.status.label("retest_all_status"),
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
            dynamic_testcases,
            (report_parent.c.dynamic_id == dynamic_testcases.c.report_id)
            & (retest_all_testcases.c.name == dynamic_testcases.c.name)
            & (
                retest_all_testcases.c.testsuite_name
                == dynamic_testcases.c.testsuite_name
            ),
        )
        .outerjoin(
            static_testcases,
            (report_parent.c.static_id == static_testcases.c.report_id)
            & (retest_all_testcases.c.name == static_testcases.c.name)
            & (
                retest_all_testcases.c.testsuite_name
                == static_testcases.c.testsuite_name
            ),
        )
        .where(
            retest_all_testcases.c.crashed == False
        )  # filter suites that are not comparable
        .where(
            (dynamic_testcases.c.crashed == None)
            | (dynamic_testcases.c.crashed == False)
        )
        .where(
            (static_testcases.c.crashed == None) | (static_testcases.c.crashed == False)
        )
        .where(retest_all_testcases.c.status != "IGNORED")
    )

    overview_parent = create_materialized_view(
        "TestCaseOverviewParent",
        overview_parent,
        # replace=True,
        metadata=Base.metadata,
    )

    testcase = DBTestCase.__table__

    testcases_count = (
        select(
            overview.c.commit,
            count(distinct(overview.c.retest_all_testcase_id)).label(
                "retest_all_count"
            ),
            count(
                distinct(
                    select(testcase.c.id)
                    .select_from(testcase)
                    .where(testcase.c.id == overview.c.retest_all_testcase_id)
                    .where(testcase.c.status == "FAILED")
                    .scalar_subquery()
                )
            ).label("retest_all_count_failed"),
            count(distinct(overview.c.dynamic_testcase_id)).label("dynamic_count"),
            count(
                distinct(
                    select(testcase.c.id)
                    .select_from(testcase)
                    .where(testcase.c.id == overview.c.dynamic_testcase_id)
                    .where(testcase.c.status == "FAILED")
                    .scalar_subquery()
                )
            ).label("dynamic_count_failed"),
            count(distinct(overview.c.static_testcase_id)).label("static_count"),
            count(
                distinct(
                    select(testcase.c.id)
                    .select_from(testcase)
                    .where(testcase.c.id == overview.c.static_testcase_id)
                    .where(testcase.c.status == "FAILED")
                    .scalar_subquery()
                )
            ).label("static_count_failed"),
        )
        .select_from(overview)
        .group_by(
            overview.c.commit,
            overview.c.retest_all_id,
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
            count(distinct(overview.c.retest_all_testcase_id)).label(
                "retest_all_count"
            ),
            count(
                distinct(
                    select(testcase.c.id)
                    .select_from(testcase)
                    .where(testcase.c.id == overview.c.retest_all_testcase_id)
                    .where(testcase.c.status == "FAILED")
                    .scalar_subquery()
                )
            ).label("retest_all_count_failed"),
            count(distinct(overview.c.dynamic_testcase_id)).label("dynamic_count"),
            count(
                distinct(
                    select(testcase.c.id)
                    .select_from(testcase)
                    .where(testcase.c.id == overview.c.dynamic_testcase_id)
                    .where(testcase.c.status == "FAILED")
                    .scalar_subquery()
                )
            ).label("dynamic_count_failed"),
            count(distinct(overview.c.static_testcase_id)).label("static_count"),
            count(
                distinct(
                    select(testcase.c.id)
                    .select_from(testcase)
                    .where(testcase.c.id == overview.c.static_testcase_id)
                    .where(testcase.c.status == "FAILED")
                    .scalar_subquery()
                )
            ).label("static_count_failed"),
        )
        .select_from(overview)
        .group_by(
            overview.c.commit,
            overview.c.target,
            overview.c.retest_all_id,
            overview.c.dynamic_id,
        )
    )

    target_count = create_materialized_view(
        "TargetCount",
        target_count,
        # replace=True,
        metadata=Base.metadata,
    )

    retest_all_selected = testcase.alias("restest_all_selected")
    dynamic_selected = testcase.alias("dynamic_selected")
    static_selected = testcase.alias("static_selected")

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

    testcase_retest_all = testcase.alias("retest_all")
    testcase_dynamic = testcase.alias("dynamic")
    testcase_static = testcase.alias("static")

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
            (overview.c.commit == overview_parent.c.commit)
            & (
                overview.c.retest_all_suite_name
                == overview_parent.c.retest_all_suite_name
            )
            & (overview.c.retest_all_name == overview_parent.c.retest_all_name),
        )
        .outerjoin(
            retest_all_selected,
            (overview.c.retest_all_testcase_id == retest_all_selected.c.id)
            # & (retest_all_selected.c.status == "FAILED"),
            & ~exists(
                select()
                .select_from(testcase_retest_all)
                .where(
                    overview_parent.c.retest_all_testcase_id == testcase_retest_all.c.id
                )
                .where(testcase_retest_all.c.status == retest_all_selected.c.status)
                .scalar_subquery()
            ),
        )
        .outerjoin(
            dynamic_selected,
            (overview.c.dynamic_testcase_id == dynamic_selected.c.id)
            # & (dynamic_selected.c.status == "FAILED"),
            & ~exists(
                select()
                .select_from(testcase_dynamic)
                .where(overview_parent.c.dynamic_testcase_id == testcase_dynamic.c.id)
                .where(testcase_dynamic.c.status == dynamic_selected.c.status)
                .scalar_subquery()
            ),
        )
        .outerjoin(
            static_selected,
            (overview.c.static_testcase_id == static_selected.c.id)
            # & (static_selected.c.status == "FAILED"),
            & ~exists(
                select()
                .select_from(testcase_static)
                .where(overview_parent.c.static_testcase_id == testcase_static.c.id)
                .where(testcase_static.c.status == static_selected.c.status)
                .scalar_subquery()
            ),
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
            func.round(
                func.cast(func.avg(report.c.retest_all_test_duration), NUMERIC), 2
            ).label("retest_all_mean"),
            func.round(
                func.cast(func.stddev(report.c.retest_all_test_duration), NUMERIC), 2
            ).label("retest_all_stddev"),
            func.round(
                func.cast(func.avg(report.c.dynamic_test_duration), NUMERIC), 2
            ).label("dynamic_mean"),
            func.round(
                func.cast(func.stddev(report.c.dynamic_test_duration), NUMERIC), 2
            ).label("dynamic_stddev"),
            func.round(
                func.cast(func.avg(report.c.static_test_duration), NUMERIC), 2
            ).label("static_mean"),
            func.round(
                func.cast(func.stddev(report.c.static_test_duration), NUMERIC), 2
            ).label("static_stddev"),
            func.round(
                func.cast(
                    func.avg(
                        report.c.dynamic_test_duration
                        * 100.0
                        / report.c.retest_all_test_duration
                    ),
                    NUMERIC,
                ),
                2,
            ).label("dynamic_mean_relative"),
            func.round(
                func.cast(
                    func.stddev(
                        report.c.dynamic_test_duration
                        * 100.0
                        / report.c.retest_all_test_duration
                    ),
                    NUMERIC,
                ),
                2,
            ).label("dynamic_stddev_relative"),
            func.round(
                func.cast(
                    func.avg(
                        report.c.static_test_duration
                        * 100.0
                        / report.c.retest_all_test_duration
                    ),
                    NUMERIC,
                ),
                2,
            ).label("static_mean_relative"),
            func.round(
                func.cast(
                    func.stddev(
                        report.c.static_test_duration
                        * 100.0
                        / report.c.retest_all_test_duration
                    ),
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
        func.round(
            func.cast(func.avg(report.c.retest_all_test_duration), NUMERIC), 2
        ).label("retest_all_mean"),
        func.round(
            func.cast(func.stddev(report.c.retest_all_test_duration), NUMERIC), 2
        ).label("retest_all_stddev"),
        func.round(
            func.cast(func.avg(report.c.dynamic_test_duration), NUMERIC), 2
        ).label("dynamic_mean"),
        func.round(
            func.cast(func.stddev(report.c.dynamic_test_duration), NUMERIC), 2
        ).label("dynamic_stddev"),
        func.round(
            func.cast(func.avg(report.c.static_test_duration), NUMERIC), 2
        ).label("static_mean"),
        func.round(
            func.cast(func.stddev(report.c.static_test_duration), NUMERIC), 2
        ).label("static_stddev"),
        func.round(
            func.cast(
                func.avg(
                    report.c.dynamic_test_duration
                    * 100.0
                    / report.c.retest_all_test_duration
                ),
                NUMERIC,
            ),
            2,
        ).label("dynamic_mean_relative"),
        func.round(
            func.cast(
                func.stddev(
                    report.c.dynamic_test_duration
                    * 100.0
                    / report.c.retest_all_test_duration
                ),
                NUMERIC,
            ),
            2,
        ).label("dynamic_stddev_relative"),
        func.round(
            func.cast(
                func.avg(
                    report.c.static_test_duration
                    * 100.0
                    / report.c.retest_all_test_duration
                ),
                NUMERIC,
            ),
            2,
        ).label("static_mean_relative"),
        func.round(
            func.cast(
                func.stddev(
                    report.c.static_test_duration
                    * 100.0
                    / report.c.retest_all_test_duration
                ),
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

    return HistoryViewInformation(
        overview,
        duration,
        report,
        target_count,
        testcases_count,
        testcases_different,
        testcases_selected,
        statistics_commit,
    )
