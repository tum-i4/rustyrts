import logging
import os
from os.path import abspath
from typing import Optional, Dict

from .cargo_base import CargoHook
from ...db.base import DBConnection
from ...models.scm.git import GitClient
from ...models.scm.base import Repository
from ...util.logging.logger import get_logger

_LOGGER = get_logger(__name__)


class CargoTestHook(CargoHook):
    def __init__(
        self,
        repository: Repository,
        git_client: GitClient,
        connection: DBConnection,
        env_vars: Optional[Dict] = None,
        build_options=None,
        test_options=None,
        report_name: Optional[str] = None,
        output_path: Optional[str] = None,
    ):
        super().__init__(
            repository,
            git_client,
            connection,
            env_vars,
            build_options,
            test_options,
            report_name,
            output_path,
        )

    def test_command_parent(self, individual_build_options, individual_test_options):
        build_options = " ".join([self.build_options, individual_build_options])
        test_options = " ".join([self.test_options, individual_test_options])
        return "cargo test {0} --no-fail-fast -- {1}".format(
            build_options,
            test_options,
        )

    def test_command(self, individual_build_options, individual_test_options):
        build_options = " ".join([self.build_options, individual_build_options])
        test_options = " ".join([self.test_options, individual_test_options])
        return "cargo test {0} -Z no-index-update --no-fail-fast -- {1}".format(
            build_options,
            test_options,
        )
