import os
import re

from .cargo_test import CargoTestTestReportLoader
from ..mutants import Mutant, MutantsTestSuite

from ..loader import TestReportLoader


class CargoMutantsTestReportLoader(TestReportLoader):

    def __init__(
            self,
            path: str,
            load_ignored: bool = True
    ):
        """
        Constructor.

        :param input: Input for loading test, should contain test events in json format
        """
        super().__init__()
        self.path = path
        if not self.path.endswith("log"):
            self.path += os.path.sep + "log"
        self.load_ignored = load_ignored

    def load(self) -> list[Mutant]:
        mutants = []

        for file in os.listdir(self.path):
            f = open(self.path + os.path.sep + file, "r")
            content = f.read()
            elements = re.split(r"^\*\*\* ", content, flags=re.MULTILINE)[1:]

            is_baseline = file == "baseline.log"

            descr = elements[0] if len(elements) > 0 else None
            diff = None
            check_log = None
            check_result = None
            check_duration = None
            test_log = None
            test_result = None
            test_duration = None
            suites = []

            offset = 0 if is_baseline else 1

            if not is_baseline:
                diff = elements[1] if len(elements) > 1 else None

            if len(elements) > (2 + offset):
                check_result = re.search(r"^cargo result: (.*) in ", elements[2 + offset], flags=re.MULTILINE).group(1)
                check_duration = re.search(r"^cargo result: .* in (.*)s", elements[2 + offset], flags=re.MULTILINE).group(1)
                check_log = elements[1 + offset].replace("\x00", "")

            if len(elements) > (4 + offset):
                test_result = re.search(r"^cargo result: (.*) in ", elements[4 + offset], flags=re.MULTILINE).group(1)
                test_duration = re.search(r"^cargo result: .* in (.*)s", elements[4 + offset], flags=re.MULTILINE).group(1)
                test_log = elements[3 + offset].replace("\x00", "")

            if test_log:
                test_loader = CargoTestTestReportLoader(test_log)
                try:
                    suites = [MutantsTestSuite.from_test_suite(suite) for suite in test_loader.load()]
                except:
                    test_log = "Failed to parse testsuites\n" + test_log

            mutant = Mutant(descr=descr,
                            diff=diff,
                            check_result=check_result,
                            check_duration=check_duration,
                            check_log=check_log,
                            test_result=test_result,
                            test_duration=test_duration,
                            test_log=test_log,
                            suites=suites)

            mutants.append(mutant)

        return mutants
