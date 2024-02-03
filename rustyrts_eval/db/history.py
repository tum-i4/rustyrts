from typing import List, Optional

from sqlalchemy import (
    Column,
    String,
    Integer,
    ForeignKey,
    Float,
    Enum,
    Text,
    UniqueConstraint,
    Boolean, Index,
)
from sqlalchemy.orm import relationship, Session

from .base import Base
from .git import DBCommit
from ..models.testing.base import TestReport, TestSuite, TestCase, TestStatus, TestTarget


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
    __table_args__ = (Index('ix_TestReport_name', "name"), Index('ix_TestsReport_commit', "commit_str"),)

    name = Column(String, nullable=False)
    duration = Column(Float)
    build_duration = Column(Float)
    suites: List["DBTestSuite"] = relationship("DBTestSuite", back_populates="report")
    commit_str = Column(String, nullable=False)
    commit_id = Column(Integer, ForeignKey("{}.id".format(DBCommit.__tablename__), ondelete="CASCADE"))
    commit: DBCommit = relationship("DBCommit", back_populates="reports")
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
            session.query(DBTestReport).filter_by(name=name, commit_str=commit_str).first()
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
            db_report.duration = report.duration if report.duration else db_report.duration
            db_report.build_duration = report.build_duration if report.build_duration else db_report.build_duration
            db_report.commit_str = report.commit_str if report.commit_str else db_report.commit_str
            # get from db if it exists
            db_report.commit = DBCommit.create_or_get(report.commit, session)
            db_report.suites = (
                [DBTestSuite.from_domain(s) for s in report.suites]
                if report.suites
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
            has_errored=self.has_errored
        )


class DBTestSuite(Base, TestSuite, metaclass=DBTestSuiteMeta):
    __table_args__ = (Index('ix_TestSuite_id_report_id_name', "id", "report_id", "name"),
                      Index('ix_TestSuite_name', "name"),
                      Index('ix_TestSuite_crashed', "crashed"),)

    name = Column(String, nullable=False)
    duration = Column(Float)
    crashed = Column(Boolean)
    total_count = Column(Integer)
    passed_count = Column(Integer)
    failed_count = Column(Integer)
    ignored_count = Column(Integer)
    measured_count = Column(Integer)
    filtered_out_count = Column(Integer)
    report_id = Column(Integer, ForeignKey("{}.id".format(DBTestReport.__tablename__), ondelete="CASCADE"))
    report = relationship("DBTestReport", back_populates="suites")
    cases: List["DBTestCase"] = relationship("DBTestCase", back_populates="suite", cascade="all, delete-orphan")

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
            filtered_out_count=suite.filtered_out_count
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
            filtered_out_count=self.filtered_out_count
        )


class DBTestCase(Base, TestCase, metaclass=DBTestCaseMeta):
    __table_args__ = (Index('ix_TestCase_id_suite_id_status', "id", "suite_id", "status"),
                      Index('ix_TestCase_name', "name"),
                      Index('ix_TestCase_status', "status"),)

    name = Column(String, nullable=True)
    target = Column(Enum(TestTarget))
    status = Column(Enum(TestStatus))
    duration = Column(Float)
    suite_id = Column(Integer, ForeignKey("{}.id".format(DBTestSuite.__tablename__), ondelete="CASCADE"))
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
