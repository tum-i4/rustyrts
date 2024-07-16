import os
from enum import Enum
from typing import Optional, Dict

from .cargo_base import CargoHook
from ...db.base import DBConnection
from ...models.scm.git import GitClient
from ...models.scm.base import Repository

from os.path import abspath, expanduser


class RustyRTSMode(str, Enum):
    DYNAMIC = "dynamic"
    STATIC = "static"


class CargoRustyRTSHook(CargoHook):
    def __init__(
        self,
        repository: Repository,
        git_client: GitClient,
        mode: RustyRTSMode,
        connection: DBConnection,
        env_vars: Optional[dict[str, str]] = None,
        build_options=None,
        rustc_options=None,
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

        self.mode = mode

    def build_env(self):
        env = super().build_env()
        if self.mode is RustyRTSMode.DYNAMIC:
            env["RUSTC"] = abspath(expanduser("~/.cargo/bin/rustyrts-dynamic"))
            env["RUSTYRTS_ONLY_INSTRUMENTATION"] = "true"
            env["CARGO_TARGET_DIR"] = "target"
        return env

    def test_command_parent(self, individual_build_options, individual_test_options):
        build_options = " ".join(self.build_options + individual_build_options)
        test_options = " ".join(self.test_options + individual_test_options)
        return "cargo rustyrts {0} {1} -- {2}".format(
            self.mode,
            build_options,
            test_options,
        )

    def test_command(self, individual_build_options, individual_test_options):
        build_options = " ".join(self.build_options + individual_build_options)
        test_options = " ".join(self.test_options + individual_test_options)
        return "cargo rustyrts {0} -Z no-index-update {1} -- {2}".format(
            self.mode,
            build_options,
            test_options,
        )
