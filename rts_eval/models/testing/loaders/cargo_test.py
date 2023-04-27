import json
import re
from json import JSONDecodeError

from ..base import TestSuite
from ..loader import TestReportLoader


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
        all_test_events = [json.loads(line) for line in self.input.splitlines() if line.startswith("{") and line.endswith("}")]
        all_test_events = [event for event in all_test_events if
                           "type" in event
                           and (event["type"] == "suite" or event["type"] == "test")
                           and event["event"] != "started"]
        if not self.load_ignored:
            all_test_events = [event for event in all_test_events if event["event"] != "ignored"]

        test_suites = list()

        all_tests_iter = all_test_events.__iter__()
        names_iter = names.__iter__()

        event = next(all_tests_iter, None)
        while event is not None:
            suite_events = list()
            while event is not None and event["type"] != "suite":
                # take until started event
                suite_events.append(event)
                event = next(all_tests_iter, None)

            suite_dict = event
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

