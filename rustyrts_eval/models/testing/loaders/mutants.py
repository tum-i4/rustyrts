import os
import re
import gc

from rustyrts_eval.db.mutants import DBMutant
from rustyrts_eval.util.logging.logger import get_logger
from .cargo_test import CargoTestTestReportLoader
from ..mutants import Mutant, MutantsTestSuite

from ..loader import TestReportLoader

_LOGGER = get_logger(__name__)


class CargoMutantsTestReportLoader:
    def __init__(
        self,
        path: str,
    ):
        """
        Constructor.

        :param path: directory containing the logs of all mutants
        """
        super().__init__()
        self.path = path
        if not self.path.endswith("log"):
            self.path += os.path.sep + "log"

    def load_mutants(self, test_report_id, connection):
        for file in os.listdir(self.path):
            freed = gc.collect()
            # _LOGGER.info("gc has freed " + str(freed) + " objects")

            _LOGGER.debug("Parsing file " + file)
            f = open(self.path + os.path.sep + file, "r")
            content = f.read()
            elements = re.split(r"^\*\*\* ", content, flags=re.MULTILINE)[1:]

            is_baseline = file == "baseline.log"

            descr = elements[0].splitlines()[0] if len(elements) > 0 else None
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
                check_result = re.search(r"^result: (.*) in ", elements[2 + offset], flags=re.MULTILINE).group(1)
                check_duration = re.search(r"^result: .* in (.*)s", elements[2 + offset], flags=re.MULTILINE).group(1)
                check_log = elements[1 + offset].replace("\x00", "")

            if len(elements) > (4 + offset):
                test_result = re.search(r"^result: (.*) in ", elements[4 + offset], flags=re.MULTILINE).group(1)
                test_duration = re.search(r"^result: .* in (.*)s", elements[4 + offset], flags=re.MULTILINE).group(1)
                test_log = elements[3 + offset].replace("\x00", "")

            # if test_log:
            #     test_loader = CargoTestTestReportLoader(test_log)
            #     try:
            #         suites = [MutantsTestSuite.from_test_suite(suite) for suite in test_loader.load()]
            #     except:
            #         test_log = "Failed to parse testsuites\n" + test_log
            #         _LOGGER.warning("Failed to parse testsuites in file " + file)

            mutant = Mutant(
                descr=descr,
                diff=diff,
                check_result=check_result,
                check_duration=check_duration,
                check_log=check_log,
                test_result=test_result,
                test_duration=test_duration,
                build_duration=CargoTestTestReportLoader.parse_build_time(test_log) if test_log else None,
                test_log=test_log,
                suites=suites,
            )

            db_mutant = DBMutant.from_domain(mutant)
            db_mutant.report_id = test_report_id

            with connection.create_session_ctx() as session:
                session.add(db_mutant)
                session.commit()
