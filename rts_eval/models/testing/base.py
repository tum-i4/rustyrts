from enum import Enum
from typing import Dict, List, Optional, Union

from ...models.scm.base import Commit


class TestStatus(str, Enum):
    """
    An enum for different statuses/results of test cases/suites.
    """

    PASSED = "ok"
    FAILED = "failed"
    IGNORED = "ignored"
    MEASURED = "measured"
    UNDEFINED = "UNDEFINED"


class TestTarget(str, Enum):
    """
    An enum for different kinds of test targets
    """

    UNDEFINED = "UNDEFINED"
    UNIT = "UNIT"
    INTEGRATION = "INTEGRATION"


class TestSuite:
    """
    A test suite contains one or more test cases and is from an implementation perspective
    often a class with multiple test methods.
    """

    def __init__(
            self,
            name: str,
            duration: float,
            cases: List["TestCase"],
            crashed: bool = False,
            total_count: Optional[int] = None,
            passed_count: Optional[int] = None,
            failed_count: Optional[int] = None,
            ignored_count: Optional[int] = None,
            measured_count: Optional[int] = None,
            filtered_out_count: Optional[int] = None,
            meta_data: Optional[str] = None
    ):
        """
        Constructor for test suites

        :param name: Unique identifier for test suite (e.g. the precise class name including the package)
        :param duration: Duration of suite execution in seconds
        :param crashed: Whether the suite has terminated with a segfault or not
        :param cases: List of test cases contained in suite
        :param total_count: Count of test cases
        :param passed_count: Count of passes
        :param failed_count: Count of failures
        :param ignored_count: Count of skips
        :param measured_count: Count of measured benches
        :param filtered_out_count: Count of excluded tests
        :param meta_data: Metadata for the test suite
        """
        self.name = name
        self.duration = duration
        self.cases = cases
        self.crashed = crashed
        self._total_count = total_count
        self._passed_count = passed_count
        self._failed_count = failed_count
        self._ignored_count = ignored_count
        self._measured_count = measured_count
        self._filtered_out_count = filtered_out_count
        self.meta_data = meta_data

    @property
    def total_count(self) -> int:
        if self._total_count:
            return self._total_count
        return len(self.cases)

    @property
    def passed_count(self) -> int:
        if self._passed_count:
            return self._passed_count
        return len(self.get_filtered_cases(status=TestStatus.PASSED))

    @property
    def failed_count(self) -> int:
        if self._failed_count:
            return self._failed_count
        return len(self.get_filtered_cases(status=TestStatus.FAILED))

    @property
    def ignored_count(self) -> int:
        if self._ignored_count:
            return self._ignored_count
        return len(self.get_filtered_cases(status=TestStatus.IGNORED))

    @property
    def measured_count(self) -> int:
        if self._measured_count:
            return self._measured_count
        return len(self.get_filtered_cases(status=TestStatus.MEASURED))

    @property
    def filtered_out_count(self) -> int:
        if self._filtered_out_count:
            return self._filtered_out_count
        return self.total_count - self.passed_count - self.failed_count - self.ignored_count - self.measured_count

    def get_setup_time(self) -> float:
        return self.duration - sum([tc.duration for tc in self.cases])

    def get_filtered_cases(self, status: TestStatus) -> List["TestCase"]:
        return list(filter(lambda tc: tc.status == status, self.cases))

    @property
    def stdout(self) -> str:
        return ",".join([tc.stdout for tc in self.cases])

    def __eq__(self, o: "TestSuite") -> bool:
        """
        Equivalence check (within test report)

        :param o:
        :return:
        """
        return self.name == o.name

    def __hash__(self) -> int:
        return hash(self.name)

    def __lt__(self, other: "TestSuite") -> bool:
        return self.name < other.name

    @classmethod
    def from_dict(cls, test_suite: Dict) -> "TestSuite":
        # we support two different kinds of JSON schemas here (one from the `ttrace` project, one from `coop`)
        return cls(
            name=test_suite["testId" if "testId" in test_suite else "name"],
            duration=test_suite["exec_time"],
            cases=(
                list(map(lambda tc: TestCase.from_dict(tc), test_suite["cases"]))
                if "cases" in test_suite
                else []
            ),
            crashed=test_suite["crashed"] if "crashed" in test_suite else False,
            total_count=(
                test_suite["test_count"] if "_total_count" in test_suite else len(test_suite["cases"])
            ),
            passed_count=(
                test_suite["passed"]
                if "passed" in test_suite
                else test_suite["_passed_count"]
            ),
            failed_count=(
                test_suite["failed"]
                if "failed" in test_suite
                else test_suite["_failed_count"]
            ),
            ignored_count=(
                test_suite["ignored"]
                if "ignored" in test_suite
                else test_suite["_ignored_count"]
            ),
            measured_count=(
                test_suite["measured"]
                if "measured" in test_suite
                else test_suite["_measured_count"]
            ),
            filtered_out_count=(
                test_suite["filtered_out"]
                if "filtered_out" in test_suite
                else test_suite["_filtered_out_count"]
            ),
        )


class TestCase:
    """
    A test case is a single test and is from an implementation perspective
    often a test methods inside a class (suite) with multiple test cases.
    """

    def __init__(
            self,
            name: str,
            target: TestTarget,
            status: TestStatus = TestStatus.UNDEFINED,
            duration: float = 0.0,
            stdout: Optional[str] = None,
    ):
        """
        Constructor for test cases

        :param name: Unique identifier for test case (e.g. the precise class name including the package + method name)
        :param target: Target of the test suite
        :param duration: Duration of case execution in seconds
        :param status: Status of the test case (i.e. passed, failed, skipped, ignored)
        :param stdout: stdout of test case
        """
        self.name = name
        self.target = target
        self.status = status
        self.duration = duration
        self.stdout = stdout

    def __eq__(self, o: "TestCase") -> bool:
        """
        Equivalence check (within test suite)

        :param o:
        :return:
        """
        return self.name == o.name

    def __hash__(self) -> int:
        """
        Test case is hashable with its name only

        :return:
        """
        return hash(self.name)

    def __repr__(self) -> str:
        """
        Print name.

        :return:
        """
        return self.name

    @classmethod
    def from_dict(cls, test_case: Dict) -> "TestCase":
        return cls(
            name=test_case["name"],
            target=TestTarget[test_case["target"]] if "target" in test_case else TestTarget.UNDEFINED,
            status=TestStatus(test_case["event"]),
            duration=test_case["exec_time"] if "exec_time" in test_case else 0.0,
            stdout=test_case["stdout"] if "stdout" in test_case else "",
        )


class TestReport:
    """
    A test report encapsulates the results of the execution of an entire test set.
    It contains test suites, which in turn contain test cases.
    """

    def __init__(
            self,
            name: str,
            duration: float,
            suites: List[TestSuite] = None,
            commit_str: Union[Optional[str], Optional[int]] = None,
            commit: Commit = None,
            log: Optional[str] = None,
            has_failed: Optional[bool] = None,
    ):
        """
        Constructor for test reports

        :param name: Unique identifier for test report (e.g. the build id)
        :param duration: Duration of complete testing procedure in seconds
        :param suites: List of test suites contained in report
        :param commit: SCM revision of test report
        :param log: Execution log
        :param has_failed: Boolean flag if exit code != 0
        """
        self.name = name
        self.duration = duration
        self.suites = suites
        self.commit_str = commit_str
        self.commit = commit
        self.log = log
        self.has_failed = has_failed

    def get_filtered_cases(self, status: TestStatus) -> List[TestCase]:
        cases = []
        for suite in self.suites:
            cases += suite.get_filtered_cases(status)
        return cases

    def __eq__(self, o: "TestReport") -> bool:
        return self.name == o.name and self.commit_str == o.commit_str
