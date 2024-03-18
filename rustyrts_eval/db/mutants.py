from typing import List, Optional

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
    distinct,
    literal_column,
    select,
    Table,
    text,
)
from sqlalchemy.sql import func
from sqlalchemy.orm import Mapped, relationship, Session
from sqlalchemy.sql.functions import coalesce, count, sum, aggregate_strings
from sqlalchemy_utils import create_materialized_view, create_view, get_columns
from sqlalchemy_utils.view import CreateView

from .base import Base
from .git import DBCommit, DBRepository
from ..models.testing.base import TestTarget, TestStatus
from ..models.testing.mutants import (
    MutantsReport,
    MutantsTestSuite,
    MutantsTestCase,
    Mutant,
    MutantsResult,
)


########################################################################################################################
# Meta classes
#


class DBMutantsReportMeta(Base.__class__, MutantsReport.__class__):
    ...


class DBMutantMeta(Base.__class__, Mutant.__class__):
    ...


class DBMutantsTestSuiteMeta(Base.__class__, MutantsTestSuite.__class__):
    ...


class DBMutantsTestCaseMeta(Base.__class__, MutantsTestCase.__class__):
    ...


########################################################################################################################
# Actual db classes
#


class DBMutantsReport(Base, MutantsReport, metaclass=DBMutantsReportMeta):
    __table_args__ = (
        Index("ix_MutantsReport_name", "name"),
        Index("ix_MutantsReport_commit", "commit_str"),
        UniqueConstraint("name", "commit_str", name="_mutants_name_revision_uc"),
    )

    name = Column(String, nullable=False)
    duration = Column(Float)
    mutants: Mapped[List["DBMutant"]] = relationship(
        "DBMutant", back_populates="report"
    )
    commit_str = Column(String, nullable=False)
    commit_id = Column(
        Integer, ForeignKey("{}.id".format(DBCommit.__tablename__), ondelete="CASCADE")
    )
    commit: Mapped[DBCommit] = relationship(
        "DBCommit", back_populates="mutants_reports"
    )
    log = Column(Text)
    has_failed = Column(Boolean)
    missed = Column(Integer)
    caught = Column(Integer)
    unviable = Column(Integer)
    timeout = Column(Integer)
    failed = Column(Integer)

    @classmethod
    def get_single(
        cls, name: str, commit_str: str, session: Session
    ) -> Optional["DBMutantsReport"]:
        db_report: Optional[DBMutantsReport] = (
            session.query(DBMutantsReport)
            .filter_by(name=name, commit_str=commit_str)
            .first()
        )
        return db_report

    @classmethod
    def create_or_update(
        cls, report: MutantsReport, session: Session
    ) -> "DBMutantsReport":
        # get report from DB
        db_report: Optional[DBMutantsReport] = cls.get_single(
            name=report.name, commit_str=report.commit_str, session=session
        )

        # create DB report object if not in DB yet
        if not db_report:
            # get commits if exists, otherwise create
            if report.commit:
                report.commit = DBCommit.create_or_get(report.commit, session)

            db_report = DBMutantsReport.from_domain(report)
            session.add(db_report)
        else:
            # if already existing, update all fields
            db_report.duration = (
                report.duration if report.duration else db_report.duration
            )

            db_report.commit_str = (
                report.commit_str if report.commit_str else db_report.commit_str
            )
            # get from db if it exists
            db_report.commit = DBCommit.create_or_get(report.commit, session)
            print("Report mutants: " + str(report.mutants))
            db_report.mutants = (
                [DBMutant.from_domain(s) for s in report.mutants]
                if report.mutants is not None
                else db_report.mutants
            )
            db_report.log = report.log if report.log else db_report.log
            db_report.has_failed = (
                report.has_failed
                if report.has_failed is not None
                else db_report.has_failed
            )
            db_report.missed = report.missed
            db_report.caught = report.caught
            db_report.unviable = report.unviable
            db_report.timeout = report.timeout
            db_report.failed = report.failed
        return db_report

    @classmethod
    def from_domain(cls, report: MutantsReport) -> "DBMutantsReport":
        if isinstance(report, cls) or not report:
            return report
        return cls(
            name=report.name,
            duration=report.duration,
            mutants=[]
            if report.mutants is None
            else [DBMutant.from_domain(mutant) for mutant in report.mutants],
            commit_str=report.commit_str,
            commit=DBCommit.from_domain(report.commit),
            log=report.log,
            has_failed=report.has_failed,
            missed=report.missed,
            caught=report.caught,
            unviable=report.unviable,
            timeout=report.timeout,
            failed=report.failed,
        )

    def to_domain(self) -> MutantsReport:
        return MutantsReport(
            name=self.name,
            duration=self.duration,
            mutants=[DBMutant.to_domain(mutant) for mutant in self.mutants],
            commit_str=self.commit_str,
            commit=self.commit.to_domain(),
            log=self.log,
            has_failed=self.has_failed,
            missed=self.missed,
            caught=self.caught,
            unviable=self.unviable,
            timeout=self.timeout,
            failed=self.failed,
        )


class DBMutant(Base, Mutant, metaclass=DBMutantMeta):
    __table_args__ = (
        Index("ix_Mutant_descr", "descr"),
        Index("ix_Mutant_report", "report_id"),
    )

    descr = Column(String, nullable=False)
    diff = Column(String, nullable=True)
    check_result = Column(Enum(MutantsResult), nullable=True)
    check_duration = Column(Float, nullable=True)
    check_log = Column(String, nullable=True)
    test_result = Column(Enum(MutantsResult), nullable=True)
    test_duration = Column(Float, nullable=True)
    build_duration = Column(Float)
    test_log = Column(String, nullable=True)
    report_id = Column(
        Integer,
        ForeignKey("{}.id".format(DBMutantsReport.__tablename__), ondelete="CASCADE"),
    )
    report = relationship("DBMutantsReport", back_populates="mutants")
    suites: Mapped[List["DBMutantsTestSuite"]] = relationship(
        "DBMutantsTestSuite", back_populates="mutant"
    )

    @classmethod
    def from_domain(cls, mutant: Mutant):
        if isinstance(mutant, cls) or not mutant:
            return mutant
        return cls(
            descr=mutant.descr,
            diff=mutant.diff,
            check_result=mutant.check_result,
            check_duration=mutant.check_duration,
            check_log=mutant.check_log,
            test_result=mutant.test_result,
            test_duration=mutant.test_duration,
            build_duration=mutant.build_duration,
            test_log=mutant.test_log,
            suites=[DBMutantsTestSuite.from_domain(suite) for suite in mutant.suites],
        )

    def to_domain(self):
        return Mutant(
            descr=self.descr,
            diff=self.diff,
            check_result=self.check_result,
            check_duration=self.check_duration,
            check_log=self.check_log,
            test_result=self.test_result,
            test_duration=self.test_duration,
            build_duration=self.build_duration,
            test_log=self.test_log,
            suites=[suite.to_domain() for suite in self.suites],
        )


class DBMutantsTestSuite(Base, MutantsTestSuite, metaclass=DBMutantsTestSuiteMeta):
    __table_args__ = (
        Index("ix_MutantsTestSuite_id_mutant_id_name", "id", "mutant_id", "name"),
        Index("ix_MutantsTestSuite_name", "name"),
        Index("ix_MutantsTestSuite_crashed", "crashed"),
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
    mutant_id = Column(
        Integer, ForeignKey("{}.id".format(DBMutant.__tablename__), ondelete="CASCADE")
    )
    mutant = relationship("DBMutant", back_populates="suites")
    cases: Mapped[List["DBMutantsTestCase"]] = relationship(
        "DBMutantsTestCase", back_populates="suite", cascade="all, delete-orphan"
    )

    @classmethod
    def from_domain(cls, suite: MutantsTestSuite) -> "DBMutantsTestSuite":
        if isinstance(suite, cls) or not suite:
            return suite
        return cls(
            name=suite.name,
            duration=suite.duration,
            crashed=suite.crashed,
            cases=[DBMutantsTestCase.from_domain(case) for case in suite.cases],
            total_count=suite.total_count,
            passed_count=suite.passed_count,
            failed_count=suite.failed_count,
            ignored_count=suite.ignored_count,
            measured_count=suite.measured_count,
            filtered_out_count=suite.filtered_out_count,
        )

    def to_domain(self) -> MutantsTestSuite:
        return MutantsTestSuite(
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


class DBMutantsTestCase(Base, MutantsTestCase, metaclass=DBMutantsTestCaseMeta):
    __table_args__ = (
        Index("ix_MutantsTestCase_id_suite_id_status", "id", "suite_id", "status"),
        Index("ix_MutantsTestCase_name", "name"),
        Index("ix_MutantsTestCase_status", "status"),
    )

    name = Column(String, nullable=True)
    target = Column(Enum(TestTarget))
    status = Column(Enum(TestStatus))
    duration = Column(Float)
    suite_id = Column(
        Integer,
        ForeignKey(
            "{}.id".format(DBMutantsTestSuite.__tablename__), ondelete="CASCADE"
        ),
    )
    suite = relationship("DBMutantsTestSuite", back_populates="cases")
    stdout = Column(String)

    @classmethod
    def from_domain(cls, case: MutantsTestCase) -> "DBMutantsTestCase":
        if isinstance(case, cls) or not case:
            return case
        return cls(
            name=case.name,
            target=case.target,
            status=case.status,
            duration=case.duration,
            stdout=case.stdout,
        )

    def to_domain(self) -> MutantsTestCase:
        return MutantsTestCase(
            name=self.name,
            target=self.target,
            status=self.status,
            duration=self.duration,
            stdout=self.stdout,
        )


########################################################################################################################
# Views


class MutantsViewInformation:
    def __init__(
        self,
        overview,
        mutant_extended,
        target_count,
        testcases_count,
        testcases_failed,
        testcases_selected,
        statistics,
    ):
        super().__init__()
        self.overview = overview
        self.mutant_extended = mutant_extended
        self.target_count = target_count
        self.testcases_count = testcases_count
        self.testcases_failed = testcases_failed
        self.testcases_selected = testcases_selected
        self.statistics = statistics

    def get_labels(self, connection, add_count=True):
        repository = DBRepository.__table__
        commit = DBCommit.__table__
        overview = self.overview

        labels_mutants = (
            select(
                repository.c.id,
                repository.c.path.concat(
                    literal_column("'\n('").concat(
                        count(distinct(overview.c.descr))
                        .label("number_mutants")
                        .concat(literal_column("')'"))
                    )
                ).label("path"),
            )
            .select_from(repository, commit, overview)
            .where(repository.c.id == commit.c.repo_id)
            .where(commit.c.id == overview.c.commit)
            .group_by(commit.c.id, repository.c.id, repository.c.path)
            .order_by(repository.c.path)
        )

        df_labels = connection.query(labels_mutants)
        df_labels["path"] = df_labels["path"].apply(lambda x: x[x.rfind("/") + 1 :])
        return df_labels


def register_views() -> MutantsViewInformation:
    commit = DBCommit.__table__

    report = DBMutantsReport.__table__
    mutant = DBMutant.__table__
    suite = DBMutantsTestSuite.__table__
    case = DBMutantsTestCase.__table__

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

    statistics = (
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
        )
        .select_from(commit, report, mutant, suite)
        .where(commit.c.id == report.c.commit_id)
        .where(report.c.name == "mutants")
        .where(mutant.c.report_id == report.c.id)
        .where(mutant.c.descr == "baseline")
        .where(suite.c.mutant_id == mutant.c.id)
        .group_by(commit.c.id, commit.c.repo_id)
    )

    statistics = create_materialized_view(
        "Statistics",
        statistics,
        # replace=True,
        metadata=Base.metadata,
    )

    retest_all = report.alias("retest_all")
    dynamic = report.alias("dynamic")
    static = report.alias("static")
    retest_all_mutant = mutant.alias("retest_all_mutant")
    dynamic_mutant = mutant.alias("dynamic_mutant")
    static_mutant = mutant.alias("static_mutant")

    mutant_extended = (
        select(
            commit.c.id.label("commit"),
            commit.c.commit_str,
            commit.c.repo_id,
            retest_all_mutant.c.descr.label("descr"),
            retest_all_mutant.c.diff.label("diff"),
            retest_all_mutant.c.id.label("retest_all_id"),
            retest_all_mutant.c.test_log.label("retest_all_test_log"),
            retest_all_mutant.c.test_result.label("retest_all_test_result"),
            retest_all_mutant.c.test_duration.label("retest_all_test_duration"),
            retest_all_mutant.c.build_duration.label("retest_all_build_duration"),
            dynamic_mutant.c.id.label("dynamic_id"),
            dynamic_mutant.c.test_log.label("dynamic_test_log"),
            dynamic_mutant.c.test_result.label("dynamic_test_result"),
            dynamic_mutant.c.test_duration.label("dynamic_test_duration"),
            dynamic_mutant.c.build_duration.label("dynamic_build_duration"),
            static_mutant.c.id.label("static_id"),
            static_mutant.c.test_log.label("static_test_log"),
            static_mutant.c.test_result.label("static_test_result"),
            static_mutant.c.test_duration.label("static_test_duration"),
            static_mutant.c.build_duration.label("static_build_duration"),
        )
        .select_from(
            commit,
            retest_all,
            dynamic,
            static,
            retest_all_mutant,
            dynamic_mutant,
            static_mutant,
        )
        .where(commit.c.id == retest_all.c.commit_id)
        .where(commit.c.id == dynamic.c.commit_id)
        .where(commit.c.id == static.c.commit_id)
        .where(retest_all_mutant.c.report_id == retest_all.c.id)
        .where(dynamic_mutant.c.report_id == dynamic.c.id)
        .where(static_mutant.c.report_id == static.c.id)
        .where(retest_all.c.name == "mutants")
        .where(dynamic.c.name == "mutants dynamic")
        .where(static.c.name == "mutants static")
        .where(retest_all_mutant.c.descr == dynamic_mutant.c.descr)
        .where(retest_all_mutant.c.descr == static_mutant.c.descr)
        .where(retest_all_mutant.c.test_log != None)
        .where(retest_all_mutant.c.test_result != "TIMEOUT")
        .where(dynamic_mutant.c.test_result != "TIMEOUT")
        .where(static_mutant.c.test_result != "TIMEOUT")
        .where(retest_all_mutant.c.descr != "baseline")
    )

    mutant_extended = create_view(
        "MutantExtended",
        mutant_extended,
        # replace=True,
        metadata=Base.metadata,
    )

    mutants_testcase_extended = (
        select(
            case,
            suite.c.crashed,
            suite.c.name.label("testsuite_name"),
            suite.c.mutant_id,
        )
        .select_from(suite, case)
        .where(suite.c.id == case.c.suite_id)
        .where(case.c.status != "IGNORED")
    )

    mutants_testcase_extended = create_view(
        "MutantsTestCaseExtended",
        mutants_testcase_extended,
        # replace=True,
        metadata=Base.metadata,
    )

    mutant = mutant_extended
    testcase = mutants_testcase_extended

    retest_all_testcases = testcase.alias("retest_all_test_cases")
    dynamic_testcases = testcase.alias("dynamic_test_cases")
    static_testcases = testcase.alias("static_test_cases")

    overview = (
        select(
            mutant.c.commit,
            mutant.c.descr.label("descr"),
            mutant.c.retest_all_id.label("retest_all_mutant_id"),
            mutant.c.dynamic_id.label("dynamic_mutant_id"),
            mutant.c.static_id.label("static_mutant_id"),
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
        .select_from(mutant)
        .join(
            retest_all_testcases,
            mutant.c.retest_all_id == retest_all_testcases.c.mutant_id,
        )
        .outerjoin(
            dynamic_testcases,
            (mutant.c.dynamic_id == dynamic_testcases.c.mutant_id)
            & (retest_all_testcases.c.name == dynamic_testcases.c.name)
            & (
                retest_all_testcases.c.testsuite_name
                == dynamic_testcases.c.testsuite_name
            ),
        )
        .outerjoin(
            static_testcases,
            (mutant.c.static_id == static_testcases.c.mutant_id)
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
        "MutantTestCaseOverview",
        overview,
        # replace=True,
        metadata=Base.metadata,
    )

    testcase = DBMutantsTestCase.__table__

    testcases_count = (
        select(
            overview.c.commit,
            overview.c.retest_all_mutant_id,
            overview.c.descr,
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
            overview.c.descr,
            overview.c.retest_all_mutant_id,
            overview.c.dynamic_mutant_id,
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
            overview.c.retest_all_mutant_id,
            overview.c.descr,
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
            overview.c.descr,
            overview.c.retest_all_mutant_id,
            overview.c.dynamic_mutant_id,
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
            overview.c.retest_all_mutant_id,
            overview.c.descr.label("descr"),
            coalesce(
                aggregate_strings(
                    retest_all_selected.c.name,
                    literal_column("'\n'"),
                ),
                "",
            ).label("retest_all"),
            coalesce(
                aggregate_strings(
                    static_selected.c.name,
                    literal_column("'\n'"),
                ),
                "",
            ).label("static"),
            coalesce(
                aggregate_strings(
                    dynamic_selected.c.name,
                    literal_column("'\n'"),
                ),
                "",
            ).label("dynamic"),
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
            overview.c.descr,
            overview.c.retest_all_mutant_id,
            overview.c.dynamic_mutant_id,
            overview.c.static_mutant_id,
        )
    )

    testcases_selected = create_materialized_view(
        "TestCasesSelected",
        testcases_selected,
        # replace=True,
        metadata=Base.metadata,
    )

    testcases_failed = (
        select(
            overview.c.commit,
            overview.c.retest_all_mutant_id,
            overview.c.descr.label("descr"),
            coalesce(
                aggregate_strings(
                    retest_all_selected.c.name,
                    literal_column("'\n'"),
                )
            ).label("retest_all"),
            coalesce(
                aggregate_strings(
                    dynamic_selected.c.name,
                    literal_column("'\n'"),
                )
            ).label("dynamic"),
            coalesce(
                aggregate_strings(
                    static_selected.c.name,
                    literal_column("'\n'"),
                )
            ).label("static"),
        )
        .select_from(overview)
        .outerjoin(
            retest_all_selected,
            (overview.c.retest_all_testcase_id == retest_all_selected.c.id)
            & (retest_all_selected.c.status == "FAILED"),
        )
        .outerjoin(
            dynamic_selected,
            (overview.c.dynamic_testcase_id == dynamic_selected.c.id)
            & (dynamic_selected.c.status == "FAILED"),
        )
        .outerjoin(
            static_selected,
            (overview.c.static_testcase_id == static_selected.c.id)
            & (static_selected.c.status == "FAILED"),
        )
        .group_by(
            overview.c.commit,
            overview.c.descr,
            overview.c.retest_all_mutant_id,
            overview.c.dynamic_mutant_id,
            overview.c.static_mutant_id,
        )
    )

    testcases_failed = create_materialized_view(
        "TestCasesFailed",
        testcases_failed,
        # replace=True,
        metadata=Base.metadata,
    )

    total_repos = select(
        literal_column("CAST('1' as int)").label("id"),
        literal_column("'MutantsTotalRepos'").label("macro"),
        func.count(distinct(mutant.c.repo_id).label("value")),
    ).select_from(mutant)
    number_mutants_total = select(
        literal_column("'2'"),
        literal_column("'MutantsNumberMutantsTotal'"),
        func.count(mutant.c.commit),
    ).select_from(mutant)

    total_retest_all = select(
        literal_column("'4'"),
        literal_column("'MutantsTotalRetestAll'"),
        func.sum(testcases_count.c.retest_all_count),
    ).select_from(testcases_count)
    total_dynamic = select(
        literal_column("'5'"),
        literal_column("'MutantsTotalDynamic'"),
        func.sum(testcases_count.c.dynamic_count),
    ).select_from(testcases_count)
    total_static = select(
        literal_column("'6'"),
        literal_column("'MutantsTotalStatic'"),
        func.sum(testcases_count.c.static_count),
    ).select_from(testcases_count)

    retest_all_failed = select(
        literal_column("'7'"),
        literal_column("'MutantsRetestAllFailed'"),
        func.sum(testcases_count.c.retest_all_count_failed),
    ).select_from(testcases_count)
    dynamic_failed = select(
        literal_column("'8'"),
        literal_column("'MutantsDynamicFailed'"),
        func.sum(testcases_count.c.dynamic_count_failed),
    ).select_from(testcases_count)
    static_failed = select(
        literal_column("'9'"),
        literal_column("'MutantsStaticFailed'"),
        func.sum(testcases_count.c.static_count_failed),
    ).select_from(testcases_count)

    relative_dynamic = select(
        literal_column("'10'"),
        literal_column("'MutantsRelativeDynamic'"),
        func.round(
            func.cast(
                func.sum(testcases_count.c.dynamic_count)
                / func.sum(testcases_count.c.retest_all_count)
                * 100.0,
                NUMERIC,
            ),
            2,
        ),
    ).select_from(testcases_count)
    relative_static = select(
        literal_column("'11'"),
        literal_column("'MutantsRelativeStatic'"),
        func.round(
            func.cast(
                func.sum(testcases_count.c.static_count)
                / func.sum(testcases_count.c.retest_all_count)
                * 100.0,
                NUMERIC,
            ),
            2,
        ),
    ).select_from(testcases_count)

    unit_retest_all = (
        select(
            literal_column("'12'"),
            literal_column("'MutantsUnitRetestAll'"),
            func.sum(target_count.c.retest_all_count),
        )
        .select_from(target_count)
        .where(target_count.c.target == "UNIT")
    )
    unit_dynamic = (
        select(
            literal_column("'13'"),
            literal_column("'MutantsUnitDynamic'"),
            func.sum(target_count.c.dynamic_count),
        )
        .select_from(target_count)
        .where(target_count.c.target == "UNIT")
    )
    unit_static = (
        select(
            literal_column("'14'"),
            literal_column("'MutantsUnitStatic'"),
            func.sum(target_count.c.static_count),
        )
        .select_from(target_count)
        .where(target_count.c.target == "UNIT")
    )

    unit_relative_dynamic = (
        select(
            literal_column("'15'"),
            literal_column("'MutantsUnitRelativeDynamic'"),
            func.round(
                func.cast(
                    func.sum(target_count.c.dynamic_count)
                    / func.sum(target_count.c.retest_all_count)
                    * 100.0,
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
            literal_column("'16'"),
            literal_column("'MutantsUnitRelativeStatic'"),
            func.round(
                func.cast(
                    func.sum(target_count.c.static_count)
                    / func.sum(target_count.c.retest_all_count)
                    * 100.0,
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
            literal_column("'17'"),
            literal_column("'MutantsIntegrationRetestAll'"),
            func.sum(target_count.c.retest_all_count),
        )
        .select_from(target_count)
        .where(target_count.c.target == "INTEGRATION")
    )
    integration_dynamic = (
        select(
            literal_column("'18'"),
            literal_column("'MutantsIntegrationDynamic'"),
            func.sum(target_count.c.dynamic_count),
        )
        .select_from(target_count)
        .where(target_count.c.target == "INTEGRATION")
    )
    integration_static = (
        select(
            literal_column("'19'"),
            literal_column("'MutantsIntegrationStatic'"),
            func.sum(target_count.c.static_count),
        )
        .select_from(target_count)
        .where(target_count.c.target == "INTEGRATION")
    )

    integration_relative_dynamic = (
        select(
            literal_column("'20'"),
            literal_column("'MutantsIntegrationRelativeDynamic'"),
            func.round(
                func.cast(
                    func.sum(target_count.c.dynamic_count)
                    / func.sum(target_count.c.retest_all_count)
                    * 100.0,
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
            literal_column("'21'"),
            literal_column("'MutantsIntegrationRelativeStatic'"),
            func.round(
                func.cast(
                    func.sum(target_count.c.static_count)
                    / func.sum(target_count.c.retest_all_count)
                    * 100.0,
                    NUMERIC,
                ),
                2,
            ),
        )
        .select_from(target_count)
        .where(target_count.c.target == "INTEGRATION")
    )

    percentage_failed_retest_all = (
        select(
            literal_column("'22'"),
            literal_column("'MutantsPercentageFailedRetestall'"),
            func.round(
                func.cast(
                    func.avg(
                        testcases_count.c.retest_all_count_failed
                        / testcases_count.c.retest_all_count
                    )
                    * 100.0,
                    NUMERIC,
                ),
                2,
            ),
        )
        .select_from(testcases_count)
        .where(testcases_count.c.retest_all_count != 0)
    )
    percentage_failed_dynamic = (
        select(
            literal_column("'23'"),
            literal_column("'MutantsPercentageFailedDynamic'"),
            func.round(
                func.cast(
                    func.avg(
                        testcases_count.c.dynamic_count_failed
                        / testcases_count.c.dynamic_count
                    )
                    * 100.0,
                    NUMERIC,
                ),
                2,
            ),
        )
        .select_from(testcases_count)
        .where(testcases_count.c.dynamic_count != 0)
    )
    percentage_failed_static = (
        select(
            literal_column("'24'"),
            literal_column("'MutantsPercentageFailedStatic'"),
            func.round(
                func.cast(
                    func.avg(
                        testcases_count.c.static_count_failed
                        / testcases_count.c.static_count
                    )
                    * 100.0,
                    NUMERIC,
                ),
                2,
            ),
        )
        .select_from(testcases_count)
        .where(testcases_count.c.static_count != 0)
    )

    facts = total_repos.union(
        number_mutants_total,
        total_retest_all,
        total_dynamic,
        total_static,
        retest_all_failed,
        dynamic_failed,
        static_failed,
        relative_dynamic,
        relative_static,
        unit_retest_all,
        unit_dynamic,
        unit_static,
        unit_relative_dynamic,
        unit_relative_static,
        integration_retest_all,
        integration_dynamic,
        integration_static,
        integration_relative_dynamic,
        integration_relative_static,
        percentage_failed_retest_all,
        percentage_failed_dynamic,
        percentage_failed_static,
    )

    facts = create_materialized_view(
        "Facts",
        facts,
        # replace=True,
        metadata=Base.metadata,
    )

    return MutantsViewInformation(
        overview,
        mutant_extended,
        target_count,
        testcases_count,
        testcases_failed,
        testcases_selected,
        statistics,
    )
