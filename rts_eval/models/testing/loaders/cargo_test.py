import json

from ..base import TestSuite
from ..loader import TestReportLoader


class CargoTestTestReportLoader(TestReportLoader):

    def __init__(
            self,
            input: str,
    ) -> None:
        """
        Constructor.

        :param input: Input for loading test, should contain test events in json format
        """
        super().__init__()
        self.input = input

    def load(self) -> list[TestSuite]:
        all_test_events = [json.loads(line) for line in self.input.splitlines() if line.startswith("{")]
        all_test_events = [event for event in all_test_events if event["event"] != "started"]
        test_suites = list()

        all_tests_iter = all_test_events.__iter__()
        event = next(all_tests_iter, None)
        while event is not None:
            suite_events = list()
            while event is not None and event["type"] != "suite":
                # take until started event
                suite_events.append(event)
                event = next(all_tests_iter, None)

            suite_dict = event
            case_dicts = suite_events

            suite_dict["name"] = "todo"
            suite_dict["cases"] = case_dicts

            suite = TestSuite.from_dict(suite_dict)
            test_suites.append(suite)

            event = next(all_tests_iter, None)

        return test_suites
