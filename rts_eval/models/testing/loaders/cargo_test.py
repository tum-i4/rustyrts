import json
import re
from json import JSONDecodeError

from ..base import TestSuite
from ..loader import TestReportLoader

IGNORE_TEST_EVENTS = ["started", "timeout"]

class CargoTestTestReportLoader(TestReportLoader):

    def __init__(
            self,
            input: str,
            load_ignored: bool = True
    ):
        """
        Constructor.

        :param input: Input for loading test, should contain test events in json format
        """
        super().__init__()
        self.input = input
        self.load_ignored = load_ignored

    def load(self) -> list[TestSuite]:

        names = re.findall(r"^ {5}Running (.*) ", self.input, re.MULTILINE)
        all_test_events = [json.loads(line) for line in self.input.splitlines() if
                           line.startswith("{") and line.endswith("}")]
        all_test_events = [event for event in all_test_events if
                           ("type" in event and "event" in event)
                           and (event["type"] == "suite" or (event["type"] == "test" and not any(
                               event["event"] == ignored for ignored in IGNORE_TEST_EVENTS)))]
        if not self.load_ignored:
            all_test_events = [event for event in all_test_events if event["event"] != "ignored"]

        test_suites = list()

        all_tests_iter = all_test_events.__iter__()
        names_iter = names.__iter__()

        event = next(all_tests_iter, None)
        while event is not None:
            suite_dict = {"passed": 0, "failed": 0, "ignored": 0, "measured": 0, "filtered_out": 0, "exec_time": 0.0}
            suite_events = list()

            if event is not None and event["event"] == "started":
                event = next(all_tests_iter, None)

            while event is not None and event["type"] != "suite":
                # take until started event
                suite_events.append(event)

                if "exec_time" in event:
                    suite_dict["exec_time"] += event["exec_time"]
                if "event" in event:
                    if event["event"] == "ok":
                        suite_dict["passed"] += 1
                    if event["event"] == "failed":
                        suite_dict["failed"] += 1

                event = next(all_tests_iter, None)

            if event is not None and event["event"] != "started":
                # The test suite did not crash
                suite_dict = event
            else:
                suite_dict["crashed"] = True

            case_dicts = suite_events

            name = names_iter.__next__()
            is_unittest = name.startswith("unittests ")
            name = name.removeprefix("unittests ")

            if is_unittest:
                for case in case_dicts:
                    case["target"] = "UNIT"
            else:
                for case in case_dicts:
                    case["target"] = "INTEGRATION"

            suite_dict["name"] = name
            suite_dict["cases"] = case_dicts

            suite = TestSuite.from_dict(suite_dict)
            test_suites.append(suite)

            event = next(all_tests_iter, None)

        return test_suites
