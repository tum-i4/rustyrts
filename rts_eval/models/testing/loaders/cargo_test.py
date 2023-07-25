import re
from json import JSONDecodeError, JSONDecoder

from ....util.logging.logger import get_logger

from ..base import TestSuite
from ..loader import TestReportLoader

_LOGGER = get_logger(__name__)

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

    @classmethod
    def parse_build_time(cls, log):
        build_times = re.finditer(r"^ {4}Finished .* in ((.*)m )?((.*)s)?", log[:log.find("Running ")], re.MULTILINE)

        count = 0
        build_time = 0

        for match in build_times:
            count += 1
            if count > 2:
                _LOGGER.warning("Found more than two build times")

            minutes = match.group(2)
            seconds = match.group(4)
            build_time += 60.0 * float(minutes) if minutes else 0.0
            build_time += float(seconds) if seconds else 0.0
        return round(build_time, 2)

    def load(self) -> list[TestSuite]:

        names = re.findall(r"^ {5}Running (.*) ", self.input, re.MULTILINE)

        all_test_events = extract_json_data(self.input)
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

            # remove "\x00" if present
            for suite_event in suite_events:
                if "stdout" in suite_event:
                    suite_event["stdout"] = suite_event["stdout"].replace("\x00", "")

            name = names_iter.__next__()
            is_unittest = name.startswith("unittests ")
            name = name.removeprefix("unittests ")

            if is_unittest:
                for case in suite_events:
                    case["target"] = "UNIT"
            else:
                for case in suite_events:
                    case["target"] = "INTEGRATION"

            suite_dict["name"] = name
            suite_dict["cases"] = suite_events

            suite = TestSuite.from_dict(suite_dict)
            test_suites.append(suite)

            event = next(all_tests_iter, None)

        return test_suites


def extract_json_data(input: str, decoder=JSONDecoder()):
    input = "".join([line for line in input.splitlines() if line.startswith("{")])
    while input:
        try:
            if "{" not in input:
                break
            input = input[input.find("{"):]
            result, index = decoder.raw_decode(input)
            yield result
            input = input[index:]
        except JSONDecodeError:
            input = input[1:]
            continue
        except ValueError:
            break
