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
    Boolean,
)
from sqlalchemy.orm import relationship, Session

from .base import Base
from .git import DBCommit
from ..models.testing.base import TestTarget, TestStatus
from ..models.testing.mutants import MutantsReport, MutantsTestSuite, MutantsTestCase, Mutant, MutantsResult


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
    name = Column(String, nullable=False)
    duration = Column(Float)
    mutants: List["DBMutant"] = relationship("DBMutant", back_populates="report")
    commit_str = Column(String, nullable=False)
    commit_id = Column(Integer, ForeignKey("{}.id".format(DBCommit.__tablename__), ondelete="CASCADE"))
    commit: DBCommit = relationship("DBCommit", back_populates="mutants_reports")
    log = Column(Text)
    has_failed = Column(Boolean)
    missed = Column(Integer)
    caught = Column(Integer)
    unviable = Column(Integer)
    timeout = Column(Integer)
    failed = Column(Integer)

    __table_args__ = tuple(
        [UniqueConstraint("name", "commit_str", name="_mutants_name_revision_uc")]
    )

    @classmethod
    def get_single(
            cls, name: str, commit_str: str, session: Session
    ) -> Optional["DBMutantsReport"]:
        db_report: Optional[DBMutantsReport] = (
            session.query(DBMutantsReport).filter_by(name=name, commit_str=commit_str).first()
        )
        return db_report

    @classmethod
    def create_or_update(cls, report: MutantsReport, session: Session) -> "DBMutantsReport":
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
            db_report.duration = report.duration if report.duration else db_report.duration

            db_report.commit_str = report.commit_str if report.commit_str else db_report.commit_str
            # get from db if it exists
            db_report.commit = DBCommit.create_or_get(report.commit, session)
            db_report.mutants = (
                [DBMutant.from_domain(s) for s in report.mutants]
                if report.mutants
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
            mutants=[] if report.mutants is None else [DBMutant.from_domain(mutant) for mutant in report.mutants],
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
    descr = Column(String, nullable=False)
    diff = Column(String, nullable=True)
    check_result = Column(Enum(MutantsResult), nullable=True)
    check_duration = Column(Float, nullable=True)
    check_log = Column(String, nullable=True)
    test_result = Column(Enum(MutantsResult), nullable=True)
    test_duration = Column(Float, nullable=True)
    build_duration = Column(Float)
    test_log = Column(String, nullable=True)
    report_id = Column(Integer, ForeignKey("{}.id".format(DBMutantsReport.__tablename__), ondelete="CASCADE"))
    report = relationship("DBMutantsReport", back_populates="mutants")
    suites: List["DBMutantsTestSuite"] = relationship("DBMutantsTestSuite", back_populates="mutant")

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
            suites=[DBMutantsTestSuite.from_domain(suite) for suite in mutant.suites]
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
            suites=[suite.to_domain() for suite in self.suites]
        )


class DBMutantsTestSuite(Base, MutantsTestSuite, metaclass=DBMutantsTestSuiteMeta):
    name = Column(String, nullable=False)
    duration = Column(Float)
    crashed = Column(Boolean)
    total_count = Column(Integer)
    passed_count = Column(Integer)
    failed_count = Column(Integer)
    ignored_count = Column(Integer)
    measured_count = Column(Integer)
    filtered_out_count = Column(Integer)
    mutant_id = Column(Integer, ForeignKey("{}.id".format(DBMutant.__tablename__), ondelete="CASCADE"))
    mutant = relationship("DBMutant", back_populates="suites")
    cases: List["DBMutantsTestCase"] = relationship("DBMutantsTestCase", back_populates="suite",
                                                    cascade="all, delete-orphan")

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
            filtered_out_count=suite.filtered_out_count
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
            filtered_out_count=self.filtered_out_count
        )


class DBMutantsTestCase(Base, MutantsTestCase, metaclass=DBMutantsTestCaseMeta):
    name = Column(String, nullable=True)
    target = Column(Enum(TestTarget))
    status = Column(Enum(TestStatus))
    duration = Column(Float)
    suite_id = Column(Integer, ForeignKey("{}.id".format(DBMutantsTestSuite.__tablename__), ondelete="CASCADE"))
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
